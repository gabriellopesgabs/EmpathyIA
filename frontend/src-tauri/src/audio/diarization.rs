use once_cell::sync::Lazy;
use std::sync::Mutex;

#[derive(Debug, Clone)]
struct SpeakerCluster {
    id: String,
    centroid_zcr: f32,
    centroid_variance: f32,
    count: usize,
}

static SPEAKERS: Lazy<Mutex<Vec<SpeakerCluster>>> = Lazy::new(|| Mutex::new(Vec::new()));

/// Simple, robust heuristic voiceprint diarizer
/// Calculates Zero-Crossing Rate (ZCR) and waveform variance to cluster different speakers
pub fn identify_speaker(samples: &[f32]) -> String {
    if samples.is_empty() {
        return "Unknown".to_string();
    }

    // 1. Calculate Zero-Crossing Rate (ZCR)
    let mut zero_crossings = 0;
    for window in samples.windows(2) {
        if (window[0] >= 0.0 && window[1] < 0.0) || (window[0] < 0.0 && window[1] >= 0.0) {
            zero_crossings += 1;
        }
    }
    let zcr = zero_crossings as f32 / samples.len() as f32;

    // 2. Calculate waveform variance (rough representation of frequency spectrum dispersion)
    let mean = samples.iter().sum::<f32>() / samples.len() as f32;
    let variance = samples.iter().map(|&x| (x - mean).powi(2)).sum::<f32>() / samples.len() as f32;

    // 3. Cluster matching using Euclidean distance of normalized voiceprint features
    let mut speakers = SPEAKERS.lock().unwrap();
    let threshold = 0.15; // Clustering threshold

    let mut best_match: Option<(usize, f32)> = None;

    for (idx, speaker) in speakers.iter().enumerate() {
        let dist = ((speaker.centroid_zcr - zcr).powi(2)
            + (speaker.centroid_variance - variance).powi(2))
        .sqrt();
        if dist < threshold {
            match best_match {
                Some((_, best_dist)) if dist < best_dist => {
                    best_match = Some((idx, dist));
                }
                None => {
                    best_match = Some((idx, dist));
                }
                _ => {}
            }
        }
    }

    if let Some((idx, _)) = best_match {
        // Update cluster centroid dynamically
        let speaker = &mut speakers[idx];
        speaker.count += 1;
        let lr = 1.0 / speaker.count as f32; // Learning rate decreases as cluster grows
        speaker.centroid_zcr += lr * (zcr - speaker.centroid_zcr);
        speaker.centroid_variance += lr * (variance - speaker.centroid_variance);
        speaker.id.clone()
    } else {
        // Create new speaker cluster
        let new_id = format!("Speaker {}", speakers.len() + 1);
        let new_cluster = SpeakerCluster {
            id: new_id.clone(),
            centroid_zcr: zcr,
            centroid_variance: variance,
            count: 1,
        };
        speakers.push(new_cluster);
        new_id
    }
}

/// Reset diarizer state for a new meeting session
pub fn reset_diarizer() {
    let mut speakers = SPEAKERS.lock().unwrap();
    speakers.clear();
}
