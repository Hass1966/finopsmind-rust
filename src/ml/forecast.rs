/// Holt-Winters / ETS time-series forecasting using the `augurs` crate.
/// Replaces Python Prophet / statsmodels.

use augurs::ets::AutoETS;
use augurs::prelude::*;

#[derive(Debug, Clone)]
pub struct ForecastResult {
    pub predicted: Vec<f64>,
    pub lower: Vec<f64>,
    pub upper: Vec<f64>,
    pub confidence: f64,
}

/// Generate a cost forecast using ETS (Exponential Smoothing).
/// `data` is the historical daily cost series, `horizon` is the number of days to predict.
pub fn generate_forecast(data: &[f64], horizon: usize) -> Result<ForecastResult, String> {
    if data.len() < 7 {
        return Err("Need at least 7 data points for forecasting".into());
    }

    let mut ets = AutoETS::non_seasonal();

    let model = ets.fit(data).map_err(|e| format!("ETS fit error: {e}"))?;
    let forecast = model.predict(horizon, 0.95).map_err(|e| format!("ETS predict error: {e}"))?;

    let point = forecast.point.clone();

    let (lower, upper) = if let Some(intervals) = forecast.intervals {
        (intervals.lower, intervals.upper)
    } else {
        // Fallback: use +/- 15% as simple confidence intervals
        let lower = point.iter().map(|v| v * 0.85).collect();
        let upper = point.iter().map(|v| v * 1.15).collect();
        (lower, upper)
    };

    // Confidence based on data length
    let confidence = (0.5 + (data.len() as f64 * 0.01)).min(0.95);

    Ok(ForecastResult {
        predicted: point,
        lower,
        upper,
        confidence,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_forecast_basic() {
        let data: Vec<f64> = (0..30)
            .map(|i| 100.0 + (i as f64 * 0.5))
            .collect();

        let result = generate_forecast(&data, 7).unwrap();
        assert_eq!(result.predicted.len(), 7);
        assert_eq!(result.lower.len(), 7);
        assert_eq!(result.upper.len(), 7);
        assert!(result.confidence > 0.5);
    }
}
