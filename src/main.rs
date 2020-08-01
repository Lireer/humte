mod data;
mod plotting;
mod util;

// use dht22_pi::ReadingError;
use log::*;
use simplelog::*;
use std::{
    collections::VecDeque,
    env, fs, io,
    str::FromStr,
    sync::{Arc, Mutex},
    thread,
};
use tiny_http::{Header, Method, Request, Response, Server, StatusCode};

const THREADS: usize = 2;

fn main() {
    let log_file = fs::File::create("./log").unwrap();
    let log_config = ConfigBuilder::new().set_time_format_str("%+").build();
    CombinedLogger::init(vec![
        TermLogger::new(LevelFilter::Debug, log_config.clone(), TerminalMode::Mixed),
        WriteLogger::new(LevelFilter::Info, log_config, log_file),
    ])
    .expect("Failed to initialize the logger");

    let mut args = env::args();
    trace!("Arguments: {:#?}", &args);

    args.next().expect("No args given");
    let pin: u8 = u8::from_str(
        &args
            .next()
            .expect("Please enter the number of the gpio pin"),
    )
    .expect("Please enter the number of the gpio pin");
    debug!("GPIO pin to read from: {}", pin);

    let addr: String = args
        .next()
        .expect("Please enter the address and port to bind to");
    debug!("Address to listen on: {}", addr);

    // start reading data from sensor
    let output_data_store = Arc::new(Mutex::new(VecDeque::with_capacity(data::MAX_READINGS)));
    let read_data_store = output_data_store.clone();
    thread::spawn(move || data::read_sensor(pin, read_data_store));
    info!("Started reading measurements from GPIO pin {}", pin);

    // start the server
    let server =
        Arc::new(Server::http(&addr).unwrap_or_else(|_| panic!("Could not listen on {}", addr)));
    let mut guards = Vec::with_capacity(THREADS);
    info!("Listening on {}", addr);

    for i in 0..THREADS {
        trace!("Starting thread {} to handle connections", i);
        let server = server.clone();
        let output_data_store = output_data_store.clone();

        let guard = thread::spawn(move || {
            for request in server.incoming_requests() {
                debug!("Handling request {:?}", request);
                if let Err(e) = handle_request(request, &output_data_store) {
                    warn!("Failed to handle a request: {:?}", e);
                }
            }
        });

        guards.push(guard);
    }

    for guard in guards {
        guard.join().unwrap();
    }
}

fn handle_request(request: Request, data: &data::DataStore) -> io::Result<()> {
    if request.method() != &Method::Get {
        trace!("Received a request which is not a GET: {:?}", request);
        return request.respond(Response::empty(StatusCode(404)));
    }

    // get the lock on the DataStore
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

    let response = Response::from_string(content)
        .with_status_code(StatusCode(200))
        .with_header(Header::from_str("Content-Type: text/html").unwrap());
    request.respond(response)
}
