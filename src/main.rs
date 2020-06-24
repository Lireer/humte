mod util;

use chrono::prelude::*;
use dht22_pi as dht;
// use dht22_pi::ReadingError;
use plotters::prelude::*;
use std::{
    collections::VecDeque,
    io::{Read, Write},
    str::FromStr,
};
use std::{env, io, fs, net, sync, thread, time};

const READ_WAIT: time::Duration = time::Duration::from_millis(1500);
const MAX_READINGS: usize = 20000;
const PLOT_PATH: &str = "./temp_hum_plot.svg";
const HTTP_NOT_FOUND: &[u8] = b"HTTP/1.1 404 Not Found\r\n\r\n";
const HTTP_GET: &[u8] = b"GET / HTTP";

type DataStore = sync::Arc<sync::Mutex<VecDeque<Data>>>;

fn main() {
    let mut args = env::args();
    args.next().expect("No args given");
    let pin: u8 = u8::from_str(
        &args
            .next()
            .expect("Please enter the number of the gpio pin"),
    )
    .expect("Please enter the number of the gpio pin");

    let addr: String = args
        .next()
        .expect("Please enter the address and port to bind to");

    // start reading data from sensor
    let output_data_store = sync::Arc::new(sync::Mutex::new(VecDeque::with_capacity(MAX_READINGS)));
    let read_data_store = output_data_store.clone();
    thread::spawn(move || read_sensor(pin, read_data_store));

    // start a server
    start_server(&addr, output_data_store)
}

#[derive(Debug)]
struct Data {
    /// Date and time when the data was measured.
    time: DateTime<Local>,
    /// Temperature in degree celsius.
    temperature: f32,
    /// Relative humidity in percent.
    rel_humidity: f32,
    /// Absolute humidity in grams per cubic meter of air.
    abs_humidity: f32,
}

impl Data {
    pub fn new(time: DateTime<Local>, temperature: f32, rel_humidity: f32) -> Self {
        Data {
            time,
            temperature,
            rel_humidity,
            abs_humidity: util::absolute_humidity(temperature, rel_humidity),
        }
    }
}

fn read_sensor(pin: u8, data_store: DataStore) {
    let mut prior_read = None;
    loop {
        match dht::read(pin) {
            Ok(read) => {
                let time = Local::now();

                // ignore the reading if temperature or humidity aren't finite
                if !read.temperature.is_finite() || !read.humidity.is_finite() {
                    break;
                }

                let new_data = Data::new(time, read.temperature, read.humidity);

                // check if the prior reading is valid given the latest stored reading and the new reading
                let mut vecd = data_store.lock().unwrap();

                if prior_read.is_some() {
                    let prior = prior_read.take().unwrap();
                    // for prior to be added to the store either the store has to be empty or prior has to be valid
                    if vecd.back().is_none()
                        || reading_is_valid(&prior, &vecd.back().unwrap(), &new_data)
                    {
                        if vecd.len() == MAX_READINGS {
                            vecd.pop_front();
                        }
                        vecd.push_back(prior);
                    } else if vecd.back().is_some() {
                        println!("{} ::> Discarded reading", time);
                        println!("store:\n{:?}", vecd.back().unwrap());
                        println!("prior:\n{:?}", prior);
                        println!("new:\n{:?}", new_data);
                    }
                }

                prior_read = Some(new_data);
            }
            _ => (),
        }
        thread::sleep(READ_WAIT);
    }
}

const OUTLIER_DIFF_MIN_TEMP: f32 = 0.3;
const OUTLIER_DIFF_MIN_HUMI: f32 = 0.4;

fn reading_is_valid(to_check: &Data, before: &Data, after: &Data) -> bool {
    // the reading is an outlier if temperature or humidity are outliers
    value_is_valid(
        to_check.temperature,
        before.temperature,
        after.temperature,
        OUTLIER_DIFF_MIN_TEMP,
    ) && value_is_valid(
        to_check.rel_humidity,
        before.rel_humidity,
        after.rel_humidity,
        OUTLIER_DIFF_MIN_HUMI,
    )
}

fn value_is_valid(to_check: f32, before: f32, after: f32, min_diff: f32) -> bool {
    let mut high = before.max(after);
    let mut low = before.min(after);
    let diff = min_diff.max(high - low);
    high += diff;
    low -= diff;

    (low..high).contains(&to_check)
}

fn start_server(addr: &str, data: DataStore) {
    let listener = net::TcpListener::bind(&addr).expect(&format!("Could not listen on {}", addr));

    for stream_result in listener.incoming() {
        if let Ok(mut stream) = stream_result {
            if let Err(_) = handle_connection(&mut stream, &data) {
                continue;
            }
        }
    }
}

fn handle_connection(stream: &mut net::TcpStream, data: &DataStore) -> io::Result<()> {
    let mut buffer = [0; 128];
    stream.read(&mut buffer)?;

    if !buffer.starts_with(HTTP_GET) {
        stream.write_all(HTTP_NOT_FOUND)?;
        stream.flush()?;
        stream.shutdown(net::Shutdown::Both)?;
        return Ok(());
    }

    // someone connected to this address
    let guard = data.lock().unwrap();

    let content = match guard.back() {
        Some(data) => {
            // generate a new plot
            generate_plot(&*guard);
            let svg = match fs::read_to_string(PLOT_PATH) {
                Ok(plot) => plot,
                Err(_) => "Plot not available".to_owned(),
            };

            format!(
                "<head>\
                    <meta charset=\"utf-8\" />\
                    <title>Temperature & Humidity</title>\
                    </head>\
                <body>\
                    <div>\
                        {time}<br/>\
                        Temperature: {temp:.1} °C<br />\
                        Relative Humidity: {rel_hum:.1} %<br />\
                        Absolute Humidity: {abs_hum:.3} g/m^3<br />\
                    </div>\
                    <div>\
                        {svg}\
                    </div>\
                </body>",
                time=data.time.format("%d.%m.%Y %H:%M:%S"),
                temp=data.temperature,
                rel_hum=data.rel_humidity,
                abs_hum=data.abs_humidity,
                svg=svg,
            )
        }
        _ => "<body>No data available</body>".to_owned(),
    };

    let response = format!("HTTP/1.1 200 OK\r\n\r\n{}", content);

    stream.write_all(response.as_bytes())?;
    stream.flush()?;

    // FIXME: This is done to avoid shutting down the connetion, while the client is
    //        still reading from the socket
    thread::sleep(time::Duration::from_millis(1000));
    stream.shutdown(net::Shutdown::Both)?;
    Ok(())
}

fn generate_plot(data: &VecDeque<Data>) -> Option<()> {
    let backend = SVGBackend::new(PLOT_PATH, (1024, 512)).into_drawing_area();
    backend.fill(&WHITE).ok()?;

    let from_date = data.front()?.time - chrono::Duration::minutes(2);
    let to_date = data.back()?.time + chrono::Duration::minutes(2);

    // Set temperature minimum and maximum
    let (temp_min, temp_max) = data.iter().fold((10000f32, -274f32), |(min, max), dp| {
        (min.min(dp.temperature), max.max(dp.temperature))
    });
    let temp_margin = 1f32.max((temp_max - temp_min) * 15.0 / 100.0);
    let temp_min = temp_min - temp_margin;
    let temp_max = temp_max + temp_margin;

    // Set relative humidity minimum and maximum
    let (hum_min, hum_max) = data.iter().fold((10000f32, -274f32), |(min, max), dp| {
        (min.min(dp.rel_humidity), max.max(dp.rel_humidity))
    });
    let hum_margin = 1f32.max((hum_max - hum_min) * 15.0 / 100.0);
    let hum_min = 0f32.max(hum_min - hum_margin);
    let hum_max = 100f32.min(hum_max + hum_margin);

    let mut chart = ChartBuilder::on(&backend)
        .margin(15)
        .x_label_area_size(40)
        .y_label_area_size(60)
        .right_y_label_area_size(60)
        .caption("Temperature & Humidity", ("sans-serif", 40).into_font())
        .build_ranged(from_date..to_date, temp_min..temp_max)
        .ok()?
        .set_secondary_coord(from_date..to_date, hum_min..hum_max);

    // Configure the mesh, and x- and y-axes
    chart
        .configure_mesh()
        .line_style_2(&WHITE)
        .axis_desc_style(("sans-serif", 25))
        .y_desc("Temperature [°C]")
        .draw()
        .ok()?;

    // Configure the rel. humidity (on the right) axes
    chart
        .configure_secondary_axes()
        .axis_desc_style(("sans-serif", 25))
        .y_desc("Relative Humidity [%]")
        .draw()
        .ok()?;

    // Draw the line for temperature
    chart
        .draw_series(LineSeries::new(
            data.iter().rev().map(|d| (d.time, d.temperature)),
            &RGBColor(255, 128, 0),
        ))
        .ok()?
        .label("Temperature [°C]")
        .legend(|(x, y)| PathElement::new(vec![(x, y), (x + 20, y)], &RGBColor(255, 128, 0)));

    // Draw the line for relative humidity
    chart
        .draw_secondary_series(LineSeries::new(
            data.iter().rev().map(|d| (d.time, d.rel_humidity)),
            &BLUE,
        ))
        .ok()?
        .label("Rel. Humidity [%]")
        .legend(|(x, y)| PathElement::new(vec![(x, y), (x + 20, y)], &BLUE));

    // Set the legend in the upper right corner
    chart
        .configure_series_labels()
        .position(SeriesLabelPosition::UpperRight)
        .background_style(&RGBColor(255, 255, 255))
        .draw()
        .ok()?;

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn range_check() {
        assert!(!value_is_valid(-10.01, 0.0, 10.0, 10.0)); // slightly below the valid range
        assert!(value_is_valid(-10.0, 0.0, 10.0, 10.0)); // lowest valid value
        assert!(value_is_valid(0.0, 0.0, 10.0, 10.0)); // valid
        assert!(value_is_valid(10.0, 0.0, 10.0, 10.0)); // valid
        assert!(value_is_valid(19.99, 0.0, 10.0, 10.0)); // high valid value
        assert!(!value_is_valid(20.0, 0.0, 10.0, 10.0)); // just above the valid range
    }
}
