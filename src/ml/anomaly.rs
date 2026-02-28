/// Z-score based anomaly detection (replaces Python ML sidecar).
/// Uses rolling window statistics to detect cost anomalies.

pub struct AnomalyDetector {
    pub window_size: usize,
    pub z_threshold: f64,
}

#[derive(Debug, Clone)]
pub struct DetectedAnomaly {
    pub index: usize,
    pub value: f64,
    pub expected: f64,
    pub deviation: f64,
    pub deviation_pct: f64,
    pub score: f64,
    pub severity: String,
}

impl AnomalyDetector {
    pub fn new(sensitivity: f64) -> Self {
        // sensitivity 0.01-0.5 maps to z-score threshold
        // higher sensitivity = lower threshold = more anomalies detected
        let z_threshold = if sensitivity <= 0.0 {
            3.0
        } else if sensitivity >= 1.0 {
            1.5
        } else {
            3.0 - (sensitivity * 3.0)
        };

        Self {
            window_size: 14,
            z_threshold: z_threshold.max(1.5),
        }
    }

    pub fn detect(&self, data: &[f64]) -> Vec<DetectedAnomaly> {
        if data.len() < self.window_size + 1 {
            return Vec::new();
        }

        let mut anomalies = Vec::new();

        for i in self.window_size..data.len() {
            let window = &data[i - self.window_size..i];
            let mean = window.iter().sum::<f64>() / window.len() as f64;
            let variance = window.iter().map(|x| (x - mean).powi(2)).sum::<f64>() / window.len() as f64;
            let std_dev = variance.sqrt();

            if std_dev < 0.001 {
                continue;
            }

            let z_score = (data[i] - mean) / std_dev;

            if z_score.abs() > self.z_threshold {
                let deviation = data[i] - mean;
                let deviation_pct = if mean.abs() > 0.001 {
                    (deviation / mean) * 100.0
                } else {
                    0.0
                };

                let score = (z_score.abs() / 5.0).min(1.0);
                let severity = classify_anomaly_severity(deviation_pct.abs());

                anomalies.push(DetectedAnomaly {
                    index: i,
                    value: data[i],
                    expected: mean,
                    deviation,
                    deviation_pct,
                    score,
                    severity,
                });
            }
        }

        anomalies
    }
}

fn classify_anomaly_severity(deviation_pct: f64) -> String {
    if deviation_pct >= 100.0 {
        "critical".into()
    } else if deviation_pct >= 50.0 {
        "high".into()
    } else if deviation_pct >= 25.0 {
        "medium".into()
    } else {
        "low".into()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_spike() {
        // Need enough data points (> window_size) with slight variance, then a spike
        let mut data: Vec<f64> = (0..20).map(|i| 100.0 + (i % 3) as f64).collect();
        data.push(500.0); // large spike

        let detector = AnomalyDetector::new(0.3); // higher sensitivity
        let anomalies = detector.detect(&data);

        assert!(!anomalies.is_empty());
        assert_eq!(anomalies[0].index, 20);
    }

    #[test]
    fn test_no_anomaly_stable() {
        let data: Vec<f64> = (0..30).map(|_| 100.0).collect();
        let detector = AnomalyDetector::new(0.1);
        let anomalies = detector.detect(&data);
        assert!(anomalies.is_empty());
    }
}
