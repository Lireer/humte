mod util;

use chrono::prelude::*;
use dht22_pi as dht;
use dht22_pi::ReadingError;
use plotters::prelude::*;
use std::{collections::VecDeque, env, fs, io::Write, net, str::FromStr, sync, thread, time};

const READ_WAIT: time::Duration = time::Duration::from_millis(1500);
const MAX_READINGS: usize = 500;
const PLOT_PATH: &str = "./temp_hum_plot.svg";

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
    loop {
        match dht::read(pin) {
            Ok(read) => {
                let time = Local::now();
                if !read.temperature.is_finite() || !read.humidity.is_finite() {
                    break;
                }

                let mut vecd = data_store.lock().unwrap();
                if vecd.len() == MAX_READINGS {
                    vecd.pop_front();
                }
                vecd.push_back(Data::new(time, read.temperature, read.humidity));
            }
            Err(ReadingError::Gpio(e)) => println!("{:#?}", e),
            _ => (),
        }
        thread::sleep(READ_WAIT);
    }
}

fn start_server(addr: &str, data: DataStore) {
    let listener = net::TcpListener::bind(&addr).expect(&format!("Could not listen on {}", addr));

    let mut err_counter = 0;
    for stream in listener.incoming() {
        match stream {
            Ok(mut stream) => {
                // someone connected to this address
                let guard = data.lock().unwrap();

                let s = match guard.back() {
                    Some(data) => format!(
                        "{}\n\
                        Temperature: {:.1} C\n\
                        Relative Humdity: {:.1} %\n\
                        Absolute Humidity: {:.3} g/m^3\n",
                        data.time.format("%d.%m.%Y %H:%M:%S"),
                        data.temperature,
                        data.rel_humidity,
                        data.abs_humidity
                    ),
                    None => "No data available".to_owned(),
                };
                stream.write_all(&s.as_bytes());

                // generate a new plot
                generate_plot(&*guard);
                if let Ok(plot) = fs::read(PLOT_PATH) {
                    stream.write_all(&plot);
                }

                stream.flush();
                // stream.shutdown(net::Shutdown::Both);
            }
            Err(e) => {
                err_counter += 1;
                if err_counter > 10 {
                    // Too many errors, something seems wrong
                    panic!("Encountered too many errors, last error: {}", e);
                }
            }
        }
    }
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
