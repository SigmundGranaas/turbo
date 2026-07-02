//! Geonorge Nedlasting API client — orders and downloads Kartverket data
//! dumps so no human ever fetches a zip by hand.
//!
//! Flow (ported from `scripts/pull-n50-fylke.sh`, proven against the live
//! API):
//!   1. `POST /api/order` with the dataset UUID + area + PostGIS/25833.
//!   2. Poll `GET /api/order/{ref}` until every file is `ReadyForDownload`
//!      (county orders are ready immediately; the national order queues).
//!   3. Stream each `downloadUrl` to the incoming dir.
//!
//! The order-body builder and area validation are pure + unit-tested; the
//! network round-trip is covered by the end-to-end provisioning test.

use std::path::{Path, PathBuf};
use std::time::Duration;

use serde_json::{json, Value};
use tokio::io::AsyncWriteExt;

use crate::job::JobError;

const API: &str = "https://nedlasting.geonorge.no/api";

/// Compiled-in default N50 metadata UUID — the fallback when the shared
/// ingestion catalog isn't mounted/parseable.
const N50_DEFAULT_UUID: &str = "ea192681-d039-42ec-b1bc-f3ce04c189ac";

/// Process-global override for the N50 metadata UUID, set once at startup from
/// the shared catalog (`set_n50_metadata_uuid`). Unset → the compiled-in
/// default, so behaviour is unchanged when the catalog is absent.
static N50_UUID_OVERRIDE: std::sync::OnceLock<String> = std::sync::OnceLock::new();

/// Point the N50 dataset at a catalog-sourced metadata UUID. Idempotent-ish:
/// only the first call wins (startup), later calls are ignored. No-op for an
/// empty/blank value so a malformed catalog can't blank the UUID.
pub fn set_n50_metadata_uuid(uuid: impl Into<String>) {
    let uuid = uuid.into();
    if !uuid.trim().is_empty() {
        let _ = N50_UUID_OVERRIDE.set(uuid);
    }
}

/// A downloadable Kartverket dataset. Adding Turrutebasen / DTM later is a
/// new variant, not a new module.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Dataset {
    /// N50 Kartdata — the topographic basemap source.
    N50,
}

impl Dataset {
    pub fn metadata_uuid(self) -> String {
        match self {
            Dataset::N50 => N50_UUID_OVERRIDE
                .get()
                .cloned()
                .unwrap_or_else(|| N50_DEFAULT_UUID.to_string()),
        }
    }
    pub fn label(self) -> &'static str {
        match self {
            Dataset::N50 => "n50",
        }
    }
}

/// Geonorge area selection: a county code (`"03"`) or the whole country.
/// The national area code in the Nedlasting codelist is `"0000"` / type
/// `landsdekkende`; everything else is a `fylke`.
#[derive(Debug, Clone)]
pub struct Area(pub String);

impl Area {
    /// Validate an operator-supplied area string. County codes are two
    /// digits; `"0000"` (or the literal `national`) selects the whole
    /// country. Rejects anything else so a typo can't become a weird order.
    pub fn parse(s: &str) -> Result<Self, JobError> {
        let s = s.trim();
        if s.eq_ignore_ascii_case("national") || s == "0000" {
            return Ok(Area("0000".to_string()));
        }
        if s.len() == 2 && s.chars().all(|c| c.is_ascii_digit()) {
            return Ok(Area(s.to_string()));
        }
        Err(JobError::Fetch(format!(
            "invalid area `{s}` (want a two-digit fylke code or `national`)"
        )))
    }

    /// `true` for the whole-country area (code `0000`).
    pub fn is_national(&self) -> bool {
        self.0 == "0000"
    }

    fn type_str(&self) -> &'static str {
        if self.is_national() {
            "landsdekkende"
        } else {
            "fylke"
        }
    }
}

/// Build the Nedlasting order JSON body. Pure — no I/O — so the request
/// shape is unit-testable without hitting the API.
pub fn build_order_body(dataset: Dataset, area: &Area) -> Value {
    json!({
        "email": "noreply@turbo.invalid",
        "softwareClient": "turbo-tileserver",
        "softwareClientVersion": env!("CARGO_PKG_VERSION"),
        "orderLines": [{
            "metadataUuid": dataset.metadata_uuid(),
            "areas": [{ "code": area.0, "type": area.type_str() }],
            "formats": [{ "name": "PostGIS" }],
            "projections": [{
                "code": "25833",
                "codespace": "http://www.opengis.net/def/crs/EPSG/0/25833"
            }]
        }]
    })
}

/// A file the order resolved to.
#[derive(Debug, Clone)]
struct OrderFile {
    download_url: String,
    name: String,
    ready: bool,
}

fn parse_order_files(resp: &Value) -> Vec<OrderFile> {
    resp.get("files")
        .and_then(|f| f.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|f| {
                    Some(OrderFile {
                        download_url: f.get("downloadUrl")?.as_str()?.to_string(),
                        name: f.get("name")?.as_str()?.to_string(),
                        ready: f
                            .get("status")
                            .and_then(|s| s.as_str())
                            .map(|s| s.eq_ignore_ascii_case("ReadyForDownload"))
                            .unwrap_or(false),
                    })
                })
                .collect()
        })
        .unwrap_or_default()
}

/// Order + download a dataset for an area into `dest_dir`. Returns the local
/// path of the (first) downloaded zip — for N50 PostGIS that's the single
/// `Basisdata_*_PostGIS.zip` the restore consumes.
pub async fn fetch(dataset: Dataset, area: &Area, dest_dir: &Path) -> Result<PathBuf, JobError> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(600))
        .user_agent(concat!("turbo-tileserver/", env!("CARGO_PKG_VERSION")))
        // Trust the OS trust store in addition to the bundled webpki roots,
        // so the fetch works behind a TLS-intercepting proxy (dev/CI) as well
        // as in a clean distroless prod image. Missing system store is a
        // no-op; webpki roots still apply.
        .tls_built_in_native_certs(true)
        .build()
        .map_err(|e| JobError::Fetch(format!("build http client: {e}")))?;

    tracing::info!(dataset = dataset.label(), area = %area.0, "geonorge: placing order");
    let order: Value = client
        .post(format!("{API}/order"))
        .json(&build_order_body(dataset, area))
        .send()
        .await
        .map_err(|e| JobError::Fetch(format!("order request: {e}")))?
        .error_for_status()
        .map_err(|e| JobError::Fetch(format!("order rejected: {e}")))?
        .json()
        .await
        .map_err(|e| JobError::Fetch(format!("order response not JSON: {e}")))?;

    let reference = order
        .get("referenceNumber")
        .and_then(|r| r.as_str())
        .ok_or_else(|| JobError::Fetch("order response missing referenceNumber".into()))?
        .to_string();

    // Poll until ready. County orders are usually ready on the first
    // response; the national order can take minutes.
    let mut files = parse_order_files(&order);
    let mut attempts = 0;
    while !files.iter().all(|f| f.ready) || files.is_empty() {
        attempts += 1;
        if attempts > 120 {
            return Err(JobError::Fetch(format!(
                "order {reference} not ready after {attempts} polls"
            )));
        }
        tokio::time::sleep(Duration::from_secs(5)).await;
        let status: Value = client
            .get(format!("{API}/order/{reference}"))
            .send()
            .await
            .map_err(|e| JobError::Fetch(format!("poll order: {e}")))?
            .json()
            .await
            .map_err(|e| JobError::Fetch(format!("poll response not JSON: {e}")))?;
        files = parse_order_files(&status);
        tracing::info!(
            reference = %reference,
            ready = files.iter().filter(|f| f.ready).count(),
            total = files.len(),
            "geonorge: polling order"
        );
    }

    tokio::fs::create_dir_all(dest_dir)
        .await
        .map_err(|e| JobError::Fetch(format!("create incoming dir: {e}")))?;

    // Download the PostGIS zip (there is exactly one per N50 PostGIS order).
    let file = files
        .into_iter()
        .find(|f| f.name.to_ascii_lowercase().ends_with(".zip"))
        .ok_or_else(|| JobError::Fetch("order produced no .zip file".into()))?;
    let dest = dest_dir.join(&file.name);

    tracing::info!(url = %file.download_url, dest = %dest.display(), "geonorge: downloading");
    let mut resp = client
        .get(&file.download_url)
        .send()
        .await
        .map_err(|e| JobError::Fetch(format!("download request: {e}")))?
        .error_for_status()
        .map_err(|e| JobError::Fetch(format!("download rejected: {e}")))?;

    let mut out = tokio::fs::File::create(&dest)
        .await
        .map_err(|e| JobError::Fetch(format!("create {}: {e}", dest.display())))?;
    let mut total: u64 = 0;
    while let Some(chunk) = resp
        .chunk()
        .await
        .map_err(|e| JobError::Fetch(format!("download stream: {e}")))?
    {
        total += chunk.len() as u64;
        out.write_all(&chunk)
            .await
            .map_err(|e| JobError::Fetch(format!("write {}: {e}", dest.display())))?;
    }
    out.flush()
        .await
        .map_err(|e| JobError::Fetch(format!("flush {}: {e}", dest.display())))?;
    tracing::info!(dest = %dest.display(), bytes = total, "geonorge: download complete");
    Ok(dest)
}

/// Kartkatalog metadata API — the cheap way to learn a dataset's published
/// data date without ordering/downloading anything. `GET {META}/{uuid}`
/// returns JSON whose `DateUpdated` is the date the *data* last changed.
const META: &str = "https://kartkatalog.geonorge.no/api/getdata";

/// Fetch the dataset's current published version marker (its `DateUpdated`)
/// from the Kartkatalog metadata endpoint. This is a single small JSON GET —
/// no order, no download — so a scheduled provision can compare it against the
/// last run's stored marker and skip the 5-7 GiB download entirely when the
/// source is unchanged.
///
/// Returns `Ok(None)` when the field is missing/blank so a metadata-shape
/// change degrades to "unknown → download" rather than an error; network/HTTP
/// failures surface as `Err` so the caller can log and fall through to the
/// (still content-hash-guarded) download path.
pub async fn fetch_metadata_version(dataset: Dataset) -> Result<Option<String>, JobError> {
    let uuid = dataset.metadata_uuid();
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(30))
        .user_agent(concat!("turbo-tileserver/", env!("CARGO_PKG_VERSION")))
        .tls_built_in_native_certs(true)
        .build()
        .map_err(|e| JobError::Fetch(format!("build metadata client: {e}")))?;
    let resp: Value = client
        .get(format!("{META}/{uuid}"))
        .send()
        .await
        .map_err(|e| JobError::Fetch(format!("metadata request: {e}")))?
        .error_for_status()
        .map_err(|e| JobError::Fetch(format!("metadata rejected: {e}")))?
        .json()
        .await
        .map_err(|e| JobError::Fetch(format!("metadata not JSON: {e}")))?;
    Ok(parse_metadata_version(&resp))
}

/// Extract the published data date from a Kartkatalog metadata response.
/// Prefers `DateUpdated` (the DATA date) over `DateMetadataUpdated` (which
/// bumps on catalog edits that don't change the data). Pure → unit-testable.
fn parse_metadata_version(resp: &Value) -> Option<String> {
    for key in [
        "DateUpdated",
        "dateUpdated",
        "DateMetadataUpdated",
        "dateMetadataUpdated",
    ] {
        if let Some(s) = resp.get(key).and_then(|v| v.as_str()) {
            let s = s.trim();
            if !s.is_empty() {
                return Some(s.to_string());
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn area_parses_counties_and_national() {
        assert_eq!(Area::parse("03").unwrap().0, "03");
        assert_eq!(Area::parse("46").unwrap().0, "46");
        assert_eq!(Area::parse("national").unwrap().0, "0000");
        assert_eq!(Area::parse("0000").unwrap().0, "0000");
        assert!(Area::parse("3").is_err());
        assert!(Area::parse("oslo").is_err());
        assert!(Area::parse("003").is_err());
    }

    #[test]
    fn area_type_is_fylke_or_landsdekkende() {
        assert_eq!(Area::parse("03").unwrap().type_str(), "fylke");
        assert_eq!(Area::parse("national").unwrap().type_str(), "landsdekkende");
    }

    #[test]
    fn order_body_targets_n50_postgis_25833() {
        let body = build_order_body(Dataset::N50, &Area::parse("03").unwrap());
        let line = &body["orderLines"][0];
        assert_eq!(line["metadataUuid"], Dataset::N50.metadata_uuid().as_str());
        assert_eq!(line["areas"][0]["code"], "03");
        assert_eq!(line["areas"][0]["type"], "fylke");
        assert_eq!(line["formats"][0]["name"], "PostGIS");
        assert_eq!(line["projections"][0]["code"], "25833");
    }

    #[test]
    fn parses_ready_files_from_an_order_response() {
        let resp = json!({
            "referenceNumber": "abc",
            "files": [
                {"name": "Basisdata_03_Oslo_25833_N50Kartdata_PostGIS.zip",
                 "downloadUrl": "https://nedlasting.geonorge.no/api/download/x",
                 "status": "ReadyForDownload"},
                {"name": "readme.txt", "downloadUrl": "https://x/y", "status": "Processing"}
            ]
        });
        let files = parse_order_files(&resp);
        assert_eq!(files.len(), 2);
        assert!(files[0].ready);
        assert!(!files[1].ready);
        assert!(files[0].name.ends_with(".zip"));
    }

    #[test]
    fn metadata_version_prefers_dateupdated() {
        let resp = json!({
            "DateUpdated": "2026-06-15",
            "DateMetadataUpdated": "2026-06-28"
        });
        assert_eq!(parse_metadata_version(&resp).as_deref(), Some("2026-06-15"));
    }

    #[test]
    fn metadata_version_falls_back_to_metadata_date() {
        let resp = json!({ "DateMetadataUpdated": "2026-06-28" });
        assert_eq!(parse_metadata_version(&resp).as_deref(), Some("2026-06-28"));
    }

    #[test]
    fn metadata_version_is_none_when_absent_or_blank() {
        assert_eq!(parse_metadata_version(&json!({})), None);
        assert_eq!(
            parse_metadata_version(&json!({ "DateUpdated": "  " })),
            None
        );
    }
}
