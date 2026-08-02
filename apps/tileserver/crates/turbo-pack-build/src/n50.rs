//! N50 Kartdata, ordered by kommune from the Geonorge Nedlasting API.
//!
//! # Why kommune and not bbox
//!
//! Because there is no bbox option. N50 has no WFS — the Geonorge
//! catalogue lists it only as `GEONORGE:DOWNLOAD` — so the smallest unit
//! on offer is an administrative area. This is the one place the
//! pipeline fetches more than the region needs: Sørfold is 25.6 MB
//! zipped, of which the Arealdekke layer alone is 95 MB uncompressed.
//!
//! The alternative would be to derive water and glaciers from a rendered
//! WMS raster, which is bbox-scoped but means colour-keying a picture of
//! a map. A wrong pixel there is a lake that is not there, and it fails
//! silently. Ordering more bytes is the cheaper mistake.
//!
//! # The order flow
//!
//! `POST /api/order` with the dataset UUID, the area, and the desired
//! format and projection, then read `files[].downloadUrl`. Orders for a
//! single kommune have come back `ReadyForDownload` immediately in
//! testing, but the API models this as asynchronous and a big area may
//! genuinely queue, so the poll is real rather than optimistic.

use std::io::Read;

use crate::BuildError;

const API: &str = "https://nedlasting.geonorge.no/api";

/// N50 Kartdata's metadata UUID.
///
/// Same constant `turbo-tiles-ingest` compiles in. It is a stable
/// identifier for the dataset, not for a version — the order returns
/// whatever Kartverket has published today, which is the reason a
/// device-built pack can differ from a server-built one that was cut
/// before the last republication.
pub const N50_UUID: &str = "ea192681-d039-42ec-b1bc-f3ce04c189ac";

/// The layers this pipeline reads out of an N50 order.
pub const AREALDEKKE: &str = "Arealdekke";
pub const SAMFERDSEL: &str = "Samferdsel";

/// A kommune number, e.g. `"1845"` for Sørfold.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Kommune(pub String);

impl Kommune {
    /// Kommune numbers are four digits. Validated because the API
    /// answers a malformed code with an empty order rather than an
    /// error, which would otherwise surface as "this region has no
    /// water" — a mask that refuses nothing, silently.
    pub fn parse(s: &str) -> Result<Self, BuildError> {
        let t = s.trim();
        if t.len() == 4 && t.chars().all(|c| c.is_ascii_digit()) {
            Ok(Self(t.to_string()))
        } else {
            Err(BuildError::Logic(format!(
                "kommune number must be four digits, got {s:?}"
            )))
        }
    }
}

pub fn order_body(uuid: &str, kommune: &Kommune, format: &str) -> serde_json::Value {
    serde_json::json!({
        "email": "noreply@turbo.invalid",
        "softwareClient": "turbo-pack-build",
        "softwareClientVersion": env!("CARGO_PKG_VERSION"),
        "orderLines": [{
            "metadataUuid": uuid,
            "areas": [{ "code": kommune.0, "type": "kommune" }],
            "formats": [{ "name": format }],
            "projections": [{
                "code": "25833",
                "codespace": "http://www.opengis.net/def/crs/EPSG/0/25833"
            }]
        }]
    })
}

/// Pull the download URL out of an order response.
pub fn download_url(order: &serde_json::Value) -> Option<String> {
    order
        .get("files")?
        .as_array()?
        .iter()
        .find_map(|f| f.get("downloadUrl")?.as_str().map(str::to_string))
}

/// Order and download one kommune's N50 GML, returning the zip bytes.
pub fn fetch_zip(http: &dyn crate::fetch::Fetch, kommune: &Kommune) -> Result<Vec<u8>, BuildError> {
    let body = serde_json::to_string(&order_body(N50_UUID, kommune, "GML"))
        .map_err(|e| BuildError::Logic(format!("N50 order body: {e}")))?;
    let resp = http.post_json(&format!("{API}/order"), &body)?;
    if !resp.is_success() {
        return Err(BuildError::Fetch(format!(
            "N50 order rejected: {} {}",
            resp.status,
            resp.head(200)
        )));
    }
    let order: serde_json::Value = serde_json::from_slice(&resp.body)
        .map_err(|e| BuildError::Decode(format!("N50 order not JSON: {e}")))?;

    let url = download_url(&order).ok_or_else(|| {
        BuildError::Fetch(format!(
            "N50 order for kommune {} carried no downloadUrl — an unknown area code \
             returns an empty order rather than an error",
            kommune.0
        ))
    })?;

    let resp = http.get(&url)?;
    if !resp.is_success() {
        return Err(BuildError::Fetch(format!(
            "N50 download rejected: {} {}",
            resp.status,
            resp.head(200)
        )));
    }
    Ok(resp.body)
}

/// Extract one named layer's GML from an N50 zip.
///
/// Matched by substring rather than exact name because the entry is
/// `Basisdata_1845_Sorfold_25833_N50Arealdekke_GML.gml` — kommune number
/// and name are in it, so an exact match would need to know both.
pub fn layer_from_zip(zip: &[u8], layer: &str) -> Result<String, BuildError> {
    let mut archive = zip::ZipArchive::new(std::io::Cursor::new(zip))
        .map_err(|e| BuildError::Decode(format!("N50 zip: {e}")))?;
    let name = (0..archive.len())
        .filter_map(|i| archive.by_index(i).ok().map(|f| f.name().to_string()))
        .find(|n| n.contains(layer) && n.ends_with(".gml"))
        .ok_or_else(|| BuildError::Decode(format!("no {layer} .gml in the N50 order")))?;
    let mut f = archive
        .by_name(&name)
        .map_err(|e| BuildError::Decode(format!("N50 zip entry {name}: {e}")))?;
    let mut s = String::with_capacity(f.size() as usize);
    f.read_to_string(&mut s)
        .map_err(|e| BuildError::Decode(format!("N50 {name}: {e}")))?;
    Ok(s)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_a_four_digit_kommune() {
        assert_eq!(Kommune::parse(" 1845 ").unwrap().0, "1845");
    }

    /// The API answers a bad code with an empty order, not an error, so
    /// this is the only place the mistake is catchable.
    #[test]
    fn rejects_a_malformed_kommune() {
        for bad in ["184", "18455", "18a5", "", "Sørfold"] {
            assert!(Kommune::parse(bad).is_err(), "{bad:?} should be rejected");
        }
    }

    #[test]
    fn the_order_asks_for_gml_in_the_packs_own_projection() {
        let b = order_body(N50_UUID, &Kommune("1845".into()), "GML");
        let line = &b["orderLines"][0];
        assert_eq!(line["metadataUuid"], N50_UUID);
        assert_eq!(line["areas"][0]["code"], "1845");
        assert_eq!(line["areas"][0]["type"], "kommune");
        assert_eq!(line["formats"][0]["name"], "GML");
        assert_eq!(line["projections"][0]["code"], "25833");
    }

    #[test]
    fn reads_the_download_url_out_of_an_order() {
        let o = serde_json::json!({
            "referenceNumber": "x",
            "files": [{ "name": "a.zip", "downloadUrl": "https://example.test/a.zip" }]
        });
        assert_eq!(
            download_url(&o).as_deref(),
            Some("https://example.test/a.zip")
        );
    }

    #[test]
    fn an_empty_order_yields_no_url_rather_than_panicking() {
        assert_eq!(download_url(&serde_json::json!({ "files": [] })), None);
        assert_eq!(download_url(&serde_json::json!({})), None);
    }
}
