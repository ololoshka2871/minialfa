#![allow(dead_code)]

use std::ops::Add;

pub struct LinearRegressionResult {
    pub k: f32,
    pub b: f32,
}

impl LinearRegressionResult {
    pub fn calc(&self, x: f32) -> f32 {
        self.k * x + self.b
    }
}

// https://www.freecodecamp.org/news/the-least-squares-regression-method-explained/
// k = sum((x - x_av) * (y - y_av))/sum((x - x_av)^2)
pub fn linear_regression(data: &[(f32, f32)]) -> LinearRegressionResult {
    let av = data
        .iter()
        .fold((0.0, 0.0), |acc, &p| (acc.0.add(p.0), acc.1.add(p.1)));

    let av = (av.0 / data.len() as f32, av.1 / data.len() as f32);

    let sums = data.iter().fold((0.0, 0.0), |acc, &p| {
        (
            acc.0.add((p.0 - av.0) * (p.1 - av.1)),
            acc.1.add((p.0 - av.0).powi(2)),
        )
    });

    let k = sums.0 / sums.1;
    let b = av.1 - k * av.0;
    LinearRegressionResult { k, b }
}
