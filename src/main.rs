use dht22_pi as dht;
use std::env;
use std::str::FromStr;

fn main() {
    let mut args = env::args();
    args.next().expect("No args given");
    let pin: u8 = u8::from_str(&args.next().expect("Please enter the number of the gpio pin")).expect("Please enter the number of the gpio pin");
    println!("Trying to read value from gpio pin {}", pin);
    println!("{:#?}", dht::read(4));
}
