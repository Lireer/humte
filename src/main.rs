mod util;

use chrono::prelude::*;
use dht22_pi as dht;
use dht22_pi::ReadingError;
use std::{collections::VecDeque, env, io::Write, net, str::FromStr, sync, thread, time};

const READ_WAIT: time::Duration = time::Duration::from_millis(1500);
const MAX_READINGS: usize = 500;

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

    // Setup
    let data = sync::Arc::new(sync::Mutex::new(VecDeque::with_capacity(MAX_READINGS)));
    let read_data = data.clone();
    let listener = net::TcpListener::bind(&addr).expect(&format!("Could not listen on {}", addr));

    thread::spawn(move || {
        loop {
            match dht::read(pin) {
                Ok(read) => {
                    let time = Local::now();
                    // TODO: Use chrono to get prettier times
                    let mut vecd = read_data.lock().unwrap();
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
    });

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
                         Absolute Humidity: {:.3} g/m^3",
                        data.time.format("%d.%m.%Y %H:%M:%S"),
                        data.temperature,
                        data.rel_humidity,
                        data.abs_humidity
                    ),
                    None => "No data available".to_owned(),
                };
                stream.write(&s.as_bytes());
                stream.flush();
                stream.shutdown(net::Shutdown::Both);
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

struct Data {
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
