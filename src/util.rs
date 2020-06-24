use std::f32::consts::E;

/// Calculates the absolute humidity in g/m^3 from the temperature and relative humidity.
///
/// Temperature in degrees celsius and relative humidity in percent.
/// For this the formular from [here](https://carnotcycle.wordpress.com/2012/08/04/how-to-convert-relative-humidity-to-absolute-humidity/)
/// is used and is supposedly accurate to within 0.1% over the temperature range -30°C to +35°C.
pub fn absolute_humidity(temp: f32, rel_hum: f32) -> f32 {
    let consts = 6.112 * 18.02 / (100.0 * 0.08314);
    let numerator = consts * rel_hum * E.powf((17.67 * temp) / (temp + 243.5));
    let denominator = 273.15 + temp;
    numerator / denominator
}
