mod data;
mod plotting;
mod util;

// use dht22_pi::ReadingError;
use std::{
    collections::VecDeque,
    env, fs,
    io::{self, Read, Write},
    net,
    str::FromStr,
    sync::{Arc, Mutex},
    thread, time,
};

const HTTP_NOT_FOUND: &[u8] = b"HTTP/1.1 404 Not Found\r\n\r\n";
const HTTP_GET: &[u8] = b"GET / HTTP";

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
    let output_data_store = Arc::new(Mutex::new(VecDeque::with_capacity(data::MAX_READINGS)));
    let read_data_store = output_data_store.clone();
    thread::spawn(move || data::read_sensor(pin, read_data_store));

    // start a server
    start_server(&addr, output_data_store)
}

fn start_server(addr: &str, data: data::DataStore) {
    let listener =
        net::TcpListener::bind(&addr).unwrap_or_else(|_| panic!("Could not listen on {}", addr));

    for stream_result in listener.incoming() {
        let _ = stream_result.and_then(|mut stream| handle_connection(&mut stream, &data));
    }
}

fn handle_connection(stream: &mut net::TcpStream, data: &data::DataStore) -> io::Result<()> {
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
            plotting::generate_plot(&*guard);
            let svg = match fs::read_to_string(plotting::PLOT_PATH) {
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
                time = data.time.format("%d.%m.%Y %H:%M:%S"),
                temp = data.temperature,
                rel_hum = data.rel_humidity,
                abs_hum = data.abs_humidity,
                svg = svg,
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
