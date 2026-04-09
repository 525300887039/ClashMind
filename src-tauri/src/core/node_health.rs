use std::cmp::Ordering;

use chrono::Utc;
use serde::{Deserialize, Serialize};

use crate::{db::repo_node_health::NodeHealthAggregate, utils::time as time_utils};

const SUCCESS_RATE_WEIGHT: f64 = 0.6;
const DELAY_WEIGHT: f64 = 0.4;

const DELAY_EXCELLENT_MS: f64 = 100.0;
const DELAY_GOOD_MS: f64 = 300.0;
const DELAY_FAIR_MS: f64 = 800.0;
const DELAY_POOR_MS: f64 = 2_000.0;
const DELAY_CRITICAL_FLOOR_MS: f64 = 5_000.0;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct NodeHealthScore {
    pub node_name: String,
    pub score: f64,
    pub grade: HealthGrade,
    pub success_rate: f64,
    pub avg_delay_ms: Option<f64>,
    pub total_tests: i64,
    pub evaluated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum HealthGrade {
    Excellent,
    Good,
    Fair,
    Poor,
    Critical,
}

#[must_use]
pub fn calculate_health_score(aggregate: &NodeHealthAggregate) -> NodeHealthScore {
    let success_rate = if aggregate.total_tests <= 0 {
        0.0
    } else {
        aggregate.success_count as f64 / aggregate.total_tests as f64
    };
    let success_score = success_rate * 100.0;
    let delay_score = calculate_delay_score(aggregate.avg_delay_ms);
    let score = success_score * SUCCESS_RATE_WEIGHT + delay_score * DELAY_WEIGHT;

    NodeHealthScore {
        node_name: aggregate.node_name.clone(),
        score,
        grade: HealthGrade::from_score(score),
        success_rate,
        avg_delay_ms: aggregate.avg_delay_ms,
        total_tests: aggregate.total_tests,
        evaluated_at: time_utils::format_utc(Utc::now()),
    }
}

#[must_use]
pub fn calculate_all_health_scores(aggregates: &[NodeHealthAggregate]) -> Vec<NodeHealthScore> {
    let mut scores: Vec<NodeHealthScore> = aggregates.iter().map(calculate_health_score).collect();
    scores.sort_by(compare_scores);
    scores
}

impl HealthGrade {
    #[must_use]
    fn from_score(score: f64) -> Self {
        if score >= 90.0 {
            Self::Excellent
        } else if score >= 70.0 {
            Self::Good
        } else if score >= 50.0 {
            Self::Fair
        } else if score >= 30.0 {
            Self::Poor
        } else {
            Self::Critical
        }
    }
}

fn calculate_delay_score(avg_delay_ms: Option<f64>) -> f64 {
    let Some(delay_ms) = avg_delay_ms else {
        return 0.0;
    };

    if delay_ms <= DELAY_EXCELLENT_MS {
        100.0
    } else if delay_ms <= DELAY_GOOD_MS {
        interpolate_score(delay_ms, DELAY_EXCELLENT_MS, DELAY_GOOD_MS, 100.0, 70.0)
    } else if delay_ms <= DELAY_FAIR_MS {
        interpolate_score(delay_ms, DELAY_GOOD_MS, DELAY_FAIR_MS, 70.0, 40.0)
    } else if delay_ms <= DELAY_POOR_MS {
        interpolate_score(delay_ms, DELAY_FAIR_MS, DELAY_POOR_MS, 40.0, 10.0)
    } else if delay_ms <= DELAY_CRITICAL_FLOOR_MS {
        interpolate_score(delay_ms, DELAY_POOR_MS, DELAY_CRITICAL_FLOOR_MS, 10.0, 0.0)
    } else {
        0.0
    }
}

fn interpolate_score(value: f64, min_x: f64, max_x: f64, max_score: f64, min_score: f64) -> f64 {
    if value <= min_x {
        return max_score;
    }

    if value >= max_x {
        return min_score;
    }

    let ratio = (value - min_x) / (max_x - min_x);
    max_score + (min_score - max_score) * ratio
}

fn compare_scores(left: &NodeHealthScore, right: &NodeHealthScore) -> Ordering {
    right
        .score
        .total_cmp(&left.score)
        .then_with(|| right.success_rate.total_cmp(&left.success_rate))
        .then_with(|| compare_optional_delay(left.avg_delay_ms, right.avg_delay_ms))
        .then_with(|| left.node_name.cmp(&right.node_name))
}

fn compare_optional_delay(left: Option<f64>, right: Option<f64>) -> Ordering {
    match (left, right) {
        (Some(left), Some(right)) => left.total_cmp(&right),
        (Some(_), None) => Ordering::Less,
        (None, Some(_)) => Ordering::Greater,
        (None, None) => Ordering::Equal,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_aggregate(
        node_name: &str,
        total_tests: i64,
        success_count: i64,
        avg_delay_ms: Option<f64>,
    ) -> NodeHealthAggregate {
        NodeHealthAggregate {
            node_name: node_name.to_string(),
            total_tests,
            success_count,
            avg_delay_ms,
            min_delay_ms: avg_delay_ms.map(|delay| delay as i32),
            max_delay_ms: avg_delay_ms.map(|delay| delay as i32),
            p95_delay_ms: avg_delay_ms.map(|delay| delay as i32),
        }
    }

    #[test]
    fn calculate_health_score_handles_missing_data() {
        let score = calculate_health_score(&make_aggregate("Proxy-A", 0, 0, None));

        assert_eq!(score.node_name, "Proxy-A");
        assert_eq!(score.score, 0.0);
        assert_eq!(score.success_rate, 0.0);
        assert_eq!(score.avg_delay_ms, None);
        assert_eq!(score.grade, HealthGrade::Critical);
        assert!(!score.evaluated_at.is_empty());
    }

    #[test]
    fn calculate_health_score_handles_all_failures() {
        let score = calculate_health_score(&make_aggregate("Proxy-A", 10, 0, None));

        assert_eq!(score.score, 0.0);
        assert_eq!(score.success_rate, 0.0);
        assert_eq!(score.grade, HealthGrade::Critical);
    }

    #[test]
    fn calculate_health_score_respects_delay_boundaries() {
        let excellent = calculate_health_score(&make_aggregate("excellent", 10, 10, Some(100.0)));
        let good = calculate_health_score(&make_aggregate("good", 10, 10, Some(300.0)));
        let fair = calculate_health_score(&make_aggregate("fair", 10, 10, Some(800.0)));
        let poor = calculate_health_score(&make_aggregate("poor", 10, 10, Some(2_000.0)));
        let critical = calculate_health_score(&make_aggregate("critical", 10, 10, Some(5_000.0)));

        assert_eq!(excellent.score, 100.0);
        assert_eq!(excellent.grade, HealthGrade::Excellent);
        assert_eq!(good.score, 88.0);
        assert_eq!(good.grade, HealthGrade::Good);
        assert_eq!(fair.score, 76.0);
        assert_eq!(fair.grade, HealthGrade::Good);
        assert_eq!(poor.score, 64.0);
        assert_eq!(poor.grade, HealthGrade::Fair);
        assert_eq!(critical.score, 60.0);
        assert_eq!(critical.grade, HealthGrade::Fair);
    }

    #[test]
    fn calculate_health_score_applies_grade_thresholds() {
        assert_eq!(HealthGrade::from_score(90.0), HealthGrade::Excellent);
        assert_eq!(HealthGrade::from_score(89.999), HealthGrade::Good);
        assert_eq!(HealthGrade::from_score(70.0), HealthGrade::Good);
        assert_eq!(HealthGrade::from_score(69.999), HealthGrade::Fair);
        assert_eq!(HealthGrade::from_score(50.0), HealthGrade::Fair);
        assert_eq!(HealthGrade::from_score(49.999), HealthGrade::Poor);
        assert_eq!(HealthGrade::from_score(30.0), HealthGrade::Poor);
        assert_eq!(HealthGrade::from_score(29.999), HealthGrade::Critical);
    }

    #[test]
    fn calculate_delay_score_drops_to_zero_after_tail_window() {
        assert_eq!(calculate_delay_score(Some(2_000.0)), 10.0);
        assert_eq!(calculate_delay_score(Some(3_500.0)), 5.0);
        assert_eq!(calculate_delay_score(Some(5_000.0)), 0.0);
        assert_eq!(calculate_delay_score(Some(8_000.0)), 0.0);
    }

    #[test]
    fn calculate_all_health_scores_sorts_stably() {
        let scores = calculate_all_health_scores(&[
            make_aggregate("Proxy-C", 10, 8, Some(150.0)),
            make_aggregate("Proxy-B", 10, 8, Some(140.0)),
            make_aggregate("Proxy-A", 10, 9, Some(200.0)),
            make_aggregate("Proxy-D", 10, 8, None),
        ]);

        let ordered_names: Vec<&str> = scores
            .iter()
            .map(|score| score.node_name.as_str())
            .collect();
        assert_eq!(
            ordered_names,
            vec!["Proxy-A", "Proxy-B", "Proxy-C", "Proxy-D"]
        );
    }
}
