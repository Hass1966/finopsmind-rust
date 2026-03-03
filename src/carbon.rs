//! Carbon emissions estimation from cloud spend.
//!
//! Uses published AWS region grid-carbon-intensity data (tCO2e per kWh)
//! and a rough heuristic: $1 of compute spend ≈ 0.10 kWh.

use chrono::NaiveDate;
use serde::Serialize;
use std::collections::HashMap;

/// Metric tonnes CO2 equivalent per kWh for AWS regions.
/// Sources: <https://docs.aws.amazon.com/awsaccountbilling/latest/aboutv2/ccft-estimation.html>
/// and US EPA eGRID / European Environment Agency data.
fn region_coefficients() -> HashMap<&'static str, f64> {
    HashMap::from([
        // North America
        ("us-east-1", 0.000_379),      // N. Virginia
        ("us-east-2", 0.000_410),      // Ohio
        ("us-west-1", 0.000_220),      // N. California
        ("us-west-2", 0.000_080),      // Oregon (largely hydro)
        ("ca-central-1", 0.000_013),   // Canada (largely hydro/nuclear)
        // Europe
        ("eu-west-1", 0.000_316),      // Ireland
        ("eu-west-2", 0.000_225),      // London
        ("eu-west-3", 0.000_056),      // Paris (largely nuclear)
        ("eu-central-1", 0.000_338),   // Frankfurt
        ("eu-north-1", 0.000_008),     // Stockholm (largely hydro)
        ("eu-south-1", 0.000_233),     // Milan
        // Asia Pacific
        ("ap-southeast-1", 0.000_408), // Singapore
        ("ap-southeast-2", 0.000_530), // Sydney
        ("ap-northeast-1", 0.000_465), // Tokyo
        ("ap-northeast-2", 0.000_415), // Seoul
        ("ap-northeast-3", 0.000_465), // Osaka
        ("ap-south-1", 0.000_708),     // Mumbai
        // South America
        ("sa-east-1", 0.000_074),      // São Paulo
        // Middle East / Africa
        ("me-south-1", 0.000_505),     // Bahrain
        ("af-south-1", 0.000_928),     // Cape Town
    ])
}

/// Fallback coefficient when region is unknown.
const DEFAULT_COEFFICIENT: f64 = 0.000_379; // US average

/// Heuristic: $1 of compute spend ≈ this many kWh.
const KWH_PER_DOLLAR: f64 = 0.10;

/// Per-region carbon breakdown entry.
#[derive(Debug, Clone, Serialize)]
pub struct RegionCarbon {
    pub region: String,
    pub spend: f64,
    pub kwh: f64,
    pub co2_kg: f64,
    pub coefficient: f64,
}

/// Monthly trend point.
#[derive(Debug, Clone, Serialize)]
pub struct CarbonTrendPoint {
    pub month: String,
    pub co2_kg: f64,
    pub spend: f64,
}

/// Full carbon report response.
#[derive(Debug, Clone, Serialize)]
pub struct CarbonReport {
    pub total_co2_kg: f64,
    pub total_kwh: f64,
    pub total_spend: f64,
    pub currency: String,
    pub start_date: NaiveDate,
    pub end_date: NaiveDate,
    pub by_region: Vec<RegionCarbon>,
    pub trend: Vec<CarbonTrendPoint>,
}

/// Estimate carbon from region-level spend rows.
/// `region_spend` – Vec of (region, total_spend) tuples.
/// `monthly_spend` – Vec of (month_string, region, spend) tuples.
pub fn estimate(
    region_spend: &[(String, f64)],
    monthly_spend: &[(String, String, f64)],
    start_date: NaiveDate,
    end_date: NaiveDate,
) -> CarbonReport {
    let coefficients = region_coefficients();

    // Per-region breakdown
    let mut total_co2 = 0.0_f64;
    let mut total_kwh = 0.0_f64;
    let mut total_spend = 0.0_f64;

    let by_region: Vec<RegionCarbon> = region_spend
        .iter()
        .map(|(region, spend)| {
            let coeff = coefficients
                .get(region.as_str())
                .copied()
                .unwrap_or(DEFAULT_COEFFICIENT);
            let kwh = spend * KWH_PER_DOLLAR;
            // coefficient is tCO2/kWh, convert to kg: * 1000
            let co2_kg = kwh * coeff * 1000.0;
            total_co2 += co2_kg;
            total_kwh += kwh;
            total_spend += spend;
            RegionCarbon {
                region: region.clone(),
                spend: *spend,
                kwh,
                co2_kg,
                coefficient: coeff,
            }
        })
        .collect();

    // Monthly trend (aggregate across regions)
    let mut month_map: HashMap<String, (f64, f64)> = HashMap::new();
    for (month, region, spend) in monthly_spend {
        let coeff = coefficients
            .get(region.as_str())
            .copied()
            .unwrap_or(DEFAULT_COEFFICIENT);
        let kwh = spend * KWH_PER_DOLLAR;
        let co2_kg = kwh * coeff * 1000.0;
        let entry = month_map.entry(month.clone()).or_insert((0.0, 0.0));
        entry.0 += co2_kg;
        entry.1 += spend;
    }

    let mut trend: Vec<CarbonTrendPoint> = month_map
        .into_iter()
        .map(|(month, (co2_kg, spend))| CarbonTrendPoint {
            month,
            co2_kg,
            spend,
        })
        .collect();
    trend.sort_by(|a, b| a.month.cmp(&b.month));

    CarbonReport {
        total_co2_kg: total_co2,
        total_kwh,
        total_spend,
        currency: "USD".into(),
        start_date,
        end_date,
        by_region,
        trend,
    }
}
