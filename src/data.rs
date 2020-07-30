use crate::util;
use chrono::prelude::*;
use dht22_pi as dht;
use std::{collections::VecDeque, sync, thread, time};

const READ_WAIT: time::Duration = time::Duration::from_millis(1500);
pub const MAX_READINGS: usize = 20000;

pub type DataStore = sync::Arc<sync::Mutex<VecDeque<Data>>>;

#[derive(Debug)]
pub struct Data {
    /// Date and time when the data was measured.
    pub time: DateTime<Local>,
    /// Temperature in degree celsius.
    pub temperature: f32,
    /// Relative humidity in percent.
    pub rel_humidity: f32,
    /// Absolute humidity in grams per cubic meter of air.
    pub abs_humidity: f32,
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

pub fn read_sensor(pin: u8, data_store: DataStore) {
    let mut prior_read = None;
    loop {
        if let Ok(read) = dht::read(pin) {
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
