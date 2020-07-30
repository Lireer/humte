use crate::data::Data;
use plotters::prelude::*;
use std::collections::VecDeque;

pub const PLOT_PATH: &str = "./temp_hum_plot.svg";

pub fn generate_plot(data: &VecDeque<Data>) -> Option<()> {
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
        .x_label_formatter(&|time| time.format("%H:%M").to_string())
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
