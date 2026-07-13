use std::time::{SystemTime, UNIX_EPOCH};

use file_engine_cli::analyzer::analyze_root;
use file_engine_cli::proposal::{propose_for_root, ProposalStatus};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CleanlinessSnapshot {
    pub score: u8,
    pub metrics: CleanlinessMetrics,
    pub calculated_at: String,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CleanlinessMetrics {
    pub total_file_count: u64,
    pub managed_file_count: u64,
    pub unorganized_file_count: u64,
    pub deductions: Vec<CleanlinessDeduction>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CleanlinessDeduction {
    pub reason_code: String,
    pub count: u64,
    pub points: u8,
}

pub fn calculate_cleanliness_snapshot(
    root: impl AsRef<std::path::Path>,
) -> Result<CleanlinessSnapshot, String> {
    let root = root.as_ref();
    let analysis = analyze_root(root).map_err(|error| error.to_string())?;
    let proposal = propose_for_root(root).map_err(|error| error.to_string())?;

    let total_file_count = u64::try_from(analysis.files.len()).unwrap_or(u64::MAX);
    let unorganized_file_count = u64::try_from(proposal.proposals.len()).unwrap_or(u64::MAX);
    let collision_count = u64::try_from(
        proposal
            .proposals
            .iter()
            .filter(|item| item.status == ProposalStatus::DestinationExists)
            .count(),
    )
    .unwrap_or(u64::MAX);
    let skipped_count = u64::try_from(analysis.skipped_entries.len()).unwrap_or(u64::MAX);

    let managed_file_count = total_file_count.saturating_sub(unorganized_file_count);
    let deductions = cleanliness_deductions(
        total_file_count,
        unorganized_file_count,
        skipped_count,
        collision_count,
    );
    let deducted_points = deductions.iter().fold(0_u16, |total, deduction| {
        total + u16::from(deduction.points)
    });
    let score = 100_u16.saturating_sub(deducted_points).min(100) as u8;

    Ok(CleanlinessSnapshot {
        score,
        metrics: CleanlinessMetrics {
            total_file_count,
            managed_file_count,
            unorganized_file_count,
            deductions,
        },
        calculated_at: unix_ms_to_rfc3339(Some(unix_ms())),
    })
}

fn cleanliness_deductions(
    total_file_count: u64,
    unorganized_file_count: u64,
    skipped_count: u64,
    collision_count: u64,
) -> Vec<CleanlinessDeduction> {
    let mut deductions = Vec::new();

    if total_file_count > 0 && unorganized_file_count > 0 {
        let points = ratio_points(unorganized_file_count, total_file_count, 70);
        deductions.push(CleanlinessDeduction {
            reason_code: "UNORGANIZED_FILES".to_string(),
            count: unorganized_file_count,
            points,
        });
    }

    if skipped_count > 0 {
        deductions.push(CleanlinessDeduction {
            reason_code: "UNREADABLE_OR_UNSAFE_ENTRIES".to_string(),
            count: skipped_count,
            points: capped_points(skipped_count, 5, 20),
        });
    }

    if collision_count > 0 {
        deductions.push(CleanlinessDeduction {
            reason_code: "PROPOSAL_CONFLICTS".to_string(),
            count: collision_count,
            points: capped_points(collision_count, 10, 20),
        });
    }

    deductions
}

fn ratio_points(count: u64, total: u64, max_points: u8) -> u8 {
    if total == 0 || count == 0 {
        return 0;
    }
    let raw = u128::from(count) * u128::from(max_points);
    let rounded_up = raw.div_ceil(u128::from(total));
    u8::try_from(rounded_up.min(u128::from(max_points))).unwrap_or(max_points)
}

fn capped_points(count: u64, points_per_item: u8, cap: u8) -> u8 {
    let points = count.saturating_mul(u64::from(points_per_item));
    u8::try_from(points.min(u64::from(cap))).unwrap_or(cap)
}

fn unix_ms() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or(0)
}

fn unix_ms_to_rfc3339(unix_ms: Option<u128>) -> String {
    let millis = unix_ms.unwrap_or(0);
    let seconds = i64::try_from(millis / 1000).unwrap_or(i64::MAX);
    let sub_ms = u32::try_from(millis % 1000).unwrap_or(0);
    let (year, month, day, hour, minute, second) = unix_seconds_to_utc(seconds);
    format!("{year:04}-{month:02}-{day:02}T{hour:02}:{minute:02}:{second:02}.{sub_ms:03}Z")
}

fn unix_seconds_to_utc(seconds: i64) -> (i32, u32, u32, u32, u32, u32) {
    let days = seconds.div_euclid(86_400);
    let day_seconds = seconds.rem_euclid(86_400);
    let (year, month, day) = civil_from_days(days);
    let hour = u32::try_from(day_seconds / 3_600).unwrap_or(0);
    let minute = u32::try_from((day_seconds % 3_600) / 60).unwrap_or(0);
    let second = u32::try_from(day_seconds % 60).unwrap_or(0);
    (year, month, day, hour, minute, second)
}

fn civil_from_days(days: i64) -> (i32, u32, u32) {
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = mp + if mp < 10 { 3 } else { -9 };
    let year = y + if month <= 2 { 1 } else { 0 };
    (
        i32::try_from(year).unwrap_or(i32::MAX),
        u32::try_from(month).unwrap_or(1),
        u32::try_from(day).unwrap_or(1),
    )
}

#[cfg(test)]
mod tests {
    use std::fs;

    use tempfile::tempdir;

    use super::calculate_cleanliness_snapshot;

    #[test]
    fn clean_root_with_rule_targets_scores_full_marks() {
        let temp = tempdir().expect("tempdir");
        let root = temp.path();
        fs::create_dir_all(root.join("documents")).expect("documents");
        fs::create_dir_all(root.join("images")).expect("images");
        fs::write(root.join("documents").join("note.md"), "note").expect("note");
        fs::write(root.join("images").join("photo.png"), "photo").expect("photo");

        let snapshot = calculate_cleanliness_snapshot(root).expect("snapshot");

        assert_eq!(snapshot.score, 100);
        assert_eq!(snapshot.metrics.total_file_count, 2);
        assert_eq!(snapshot.metrics.managed_file_count, 2);
        assert_eq!(snapshot.metrics.unorganized_file_count, 0);
        assert!(snapshot.metrics.deductions.is_empty());
    }

    #[test]
    fn proposed_cleanup_items_reduce_cleanliness_score() {
        let temp = tempdir().expect("tempdir");
        let root = temp.path();
        fs::create_dir_all(root.join("inbox")).expect("inbox");
        fs::write(root.join("inbox").join("note.md"), "note").expect("note");
        fs::write(root.join("inbox").join("photo.png"), "photo").expect("photo");
        fs::write(root.join("inbox").join("unknown.bin"), "bin").expect("bin");

        let snapshot = calculate_cleanliness_snapshot(root).expect("snapshot");

        assert_eq!(snapshot.score, 53);
        assert_eq!(snapshot.metrics.total_file_count, 3);
        assert_eq!(snapshot.metrics.managed_file_count, 1);
        assert_eq!(snapshot.metrics.unorganized_file_count, 2);
        assert_eq!(
            snapshot.metrics.deductions[0].reason_code,
            "UNORGANIZED_FILES"
        );
        assert_eq!(snapshot.metrics.deductions[0].points, 47);
    }

    #[test]
    fn destination_collisions_are_reported_as_extra_deductions() {
        let temp = tempdir().expect("tempdir");
        let root = temp.path();
        fs::create_dir_all(root.join("inbox")).expect("inbox");
        fs::create_dir_all(root.join("documents")).expect("documents");
        fs::write(root.join("inbox").join("note.md"), "new").expect("new");
        fs::write(root.join("documents").join("note.md"), "existing").expect("existing");

        let snapshot = calculate_cleanliness_snapshot(root).expect("snapshot");

        assert_eq!(snapshot.metrics.unorganized_file_count, 1);
        assert!(snapshot
            .metrics
            .deductions
            .iter()
            .any(|deduction| deduction.reason_code == "PROPOSAL_CONFLICTS"));
    }
}
