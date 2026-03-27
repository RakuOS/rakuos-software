/// ODRS (Open Desktop Ratings Service) client.
/// API: https://odrs.gnome.org/1.0/reviews/api/
/// Same service used by GNOME Software, KDE Discover, elementary AppCenter.

use serde::{Deserialize, Serialize};

const ODRS_BASE: &str = "https://odrs.gnome.org/1.0/reviews/api";
const USER_AGENT: &str = "RakuOS-Software/1.0";

// ── Data types ────────────────────────────────────────────────────────────────

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct Review {
    #[serde(default)]
    pub review_id: i64,
    #[serde(default)]
    pub summary: String,
    #[serde(default)]
    pub description: String,
    /// 0-100 rating
    #[serde(default)]
    pub rating: i32,
    #[serde(default)]
    pub user_display: Option<String>,
    #[serde(default)]
    pub date_created: f64,  // ODRS returns this as a float (e.g. 1773880315.0)
    #[serde(default)]
    pub version: String,
    #[serde(default)]
    pub distro: String,
    #[serde(default)]
    pub karma_up: i32,
    #[serde(default)]
    pub karma_down: i32,
}

// ── User hash (same approach as gnome-software) ───────────────────────────────

/// One-way hash of machine-id + username — prevents duplicate reviews
/// without exposing the username to the server.
pub fn user_hash() -> String {
    use sha1::Digest;
    let machine_id = std::fs::read_to_string("/etc/machine-id")
        .or_else(|_| std::fs::read_to_string("/var/lib/dbus/machine-id"))
        .unwrap_or_default();
    let username = std::env::var("USER")
        .or_else(|_| std::env::var("LOGNAME"))
        .unwrap_or_else(|_| "user".to_string());
    let raw = format!("{}{}", machine_id.trim(), username);
    let digest = sha1::Sha1::digest(raw.as_bytes());
    digest.iter().map(|b| format!("{:02x}", b)).collect()
}

// ── HTTP helpers ──────────────────────────────────────────────────────────────

async fn get_json(path: &str) -> Option<serde_json::Value> {
    let url = format!("{}/{}", ODRS_BASE, path);
    let client = reqwest::Client::builder()
        .user_agent(USER_AGENT)
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .ok()?;
    let resp = client.get(&url).send().await.ok()?;
    resp.json().await.ok()
}

async fn post_json(path: &str, body: &serde_json::Value) -> Option<serde_json::Value> {
    let url = format!("{}/{}", ODRS_BASE, path);
    let client = reqwest::Client::builder()
        .user_agent(USER_AGENT)
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .ok()?;
    let resp = client.post(&url).json(body).send().await.ok()?;
    resp.json().await.ok()
}

// ── Public API ────────────────────────────────────────────────────────────────

/// Fetch reviews for an app, trying multiple ID variants to maximise hit rate.
/// Tries: as-is → with .desktop suffix → title-cased last component.
pub async fn get_reviews(app_id: &str) -> Vec<Review> {
    let mut ids = vec![app_id.to_string()];

    if !app_id.ends_with(".desktop") {
        ids.push(format!("{}.desktop", app_id));
    }

    let parts: Vec<&str> = app_id.split('.').collect();
    if parts.len() >= 2 {
        let last = parts.last().unwrap();
        let titled = format!(
            "{}.{}",
            parts[..parts.len() - 1].join("."),
            last[..1].to_uppercase() + &last[1..]
        );
        if !ids.contains(&titled) {
            ids.push(titled);
        }
    }

    for id in &ids {
        let path = format!("app/{}", urlencoded(id));
        if let Some(val) = get_json(&path).await {
            if let Ok(reviews) = serde_json::from_value::<Vec<Review>>(val) {
                if !reviews.is_empty() {
                    return reviews;
                }
            }
        }
    }
    vec![]
}

/// Compute aggregate star ratings from a reviews slice.
/// Returns `(avg_stars_1_to_5, total_count)`.
pub fn aggregate(reviews: &[Review]) -> (f32, usize) {
    if reviews.is_empty() {
        return (0.0, 0);
    }
    let total = reviews.len();
    let sum: f32 = reviews
        .iter()
        .map(|r| (r.rating as f32 / 20.0).clamp(1.0, 5.0))
        .sum();
    (sum / total as f32, total)
}

/// Submit a review. Returns `Ok(())` on success or `Err(message)`.
pub async fn submit_review(
    app_id: &str,
    summary: &str,
    description: &str,
    rating: i32,
    version: &str,
    user_display: Option<&str>,
) -> Result<(), String> {
    if summary.trim().is_empty() || description.trim().is_empty() {
        return Err("Summary and review text are required.".into());
    }
    if !(0..=100).contains(&rating) {
        return Err("Rating must be 0–100.".into());
    }

    let body = serde_json::json!({
        "user_hash":   user_hash(),
        "app_id":      app_id,
        "locale":      "en_GB",
        "summary":     summary.trim(),
        "description": description.trim(),
        "rating":      rating,
        "version":     version,
        "distro":      "RakuOS Linux",
        "user_display": user_display,
    });

    match post_json("submit", &body).await {
        None => Err("Could not reach the review server.".into()),
        Some(v) => {
            if let Some(msg) = v.get("msg").and_then(|m| m.as_str()) {
                let lower = msg.to_lowercase();
                if lower.contains("already reviewed") {
                    return Err("You have already reviewed this app.".into());
                }
                return Err(msg.to_string());
            }
            Ok(())
        }
    }
}

/// Upvote or downvote a review. Fire-and-forget; spawns its own thread.
pub fn vote_review(review_id: i64, upvote: bool) {
    std::thread::spawn(move || {
        let rt = tokio::runtime::Runtime::new().unwrap();
        let body = serde_json::json!({
            "user_hash": user_hash(),
            "review_id": review_id,
            "vote_id":   if upvote { 1 } else { -1 },
        });
        rt.block_on(async { post_json("vote", &body).await });
    });
}

// ── Helpers ───────────────────────────────────────────────────────────────────

fn urlencoded(s: &str) -> String {
    s.chars()
        .flat_map(|c| {
            if c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.' | '~') {
                vec![c]
            } else {
                format!("%{:02X}", c as u32).chars().collect()
            }
        })
        .collect()
}
