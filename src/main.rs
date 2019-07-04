use dht22_pi as dht;
use dht22_pi::ReadingError;
use std::io::Write;
use std::str::FromStr;
use std::sync::mpsc;
use std::{env, fs, thread, time};

fn main() {
    let mut args = env::args();
    args.next().expect("No args given");
    let pin: u8 = u8::from_str(
        &args
            .next()
            .expect("Please enter the number of the gpio pin"),
    )
    .expect("Please enter the number of the gpio pin");

    let (send, recv) = mpsc::channel();

    thread::spawn(move || {
        const READ_WAIT: time::Duration = time::Duration::from_secs(2);
        const TRY_WAIT: time::Duration = time::Duration::from_millis(500);
        loop {
            match dht::read(pin) {
                Ok(r) => {
                    // TODO: Use chrono to get prettier times
                    send.send((time::SystemTime::now(), r)).unwrap();
                    thread::sleep(READ_WAIT);
                }
                Err(ReadingError::Gpio(e)) => println!("{:#?}", e),
                _ => thread::sleep(TRY_WAIT),
            }
        }
    });

    let mut file = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open("data.csv")
        .unwrap();

    loop {
        let (time, reading) = recv.recv().unwrap();
        let s = format!("{:?}, {}, {}", time, reading.temperature, reading.humidity);
        println!("{}", s);
        file.write_all(s.as_bytes()).unwrap();
    }
}
