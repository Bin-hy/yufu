//! Deployment-wide presentation settings. No authentication decisions or I/O.
use super::error::IdentityError;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub const DEFAULT_JSON: &str = include_str!("../../../../brand/site-branding.defaults.json");
pub const MAX_REVISION: u64 = 9_007_199_254_740_991;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SiteBranding {
    pub site_name: String,
    pub site_description: String,
    pub logo_asset_id: Option<Uuid>,
    pub favicon_asset_id: Option<Uuid>,
    pub cover_asset_id: Option<Uuid>,
    pub welcome_title: String,
    pub welcome_description: String,
    pub footer: FooterConfig,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OidcBrandingOverride {
    pub cover_asset_id: Option<Uuid>,
    pub welcome_title: Option<String>,
    pub welcome_description: Option<String>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FooterConfig {
    pub copyright_text: String,
    pub filing_number: String,
    pub filing_url: Option<String>,
    pub links: Vec<FooterLink>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FooterLink {
    pub label: String,
    pub url: String,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SiteSettingsDocument {
    pub branding: SiteBranding,
    pub oidc: OidcBrandingOverride,
}
impl SiteSettingsDocument {
    pub fn defaults() -> Result<Self, IdentityError> {
        serde_json::from_str(DEFAULT_JSON).map_err(|_| IdentityError::DependencyUnavailable)
    }
}
#[derive(Debug, Clone, Serialize)]
pub struct SiteSettings {
    pub document: SiteSettingsDocument,
    pub revision: u64,
    pub updated_by: Option<i32>,
    pub updated_at: Option<u64>,
}
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SaveSiteSettings {
    pub expected_revision: u64,
    pub document: SiteSettingsDocument,
}
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ResetSiteSettings {
    pub expected_revision: u64,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SiteAssetKind {
    Logo,
    Favicon,
    Cover,
}
impl SiteAssetKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Logo => "logo",
            Self::Favicon => "favicon",
            Self::Cover => "cover",
        }
    }
    pub fn max_bytes(self) -> usize {
        if self == Self::Cover {
            10 * 1024 * 1024
        } else {
            2 * 1024 * 1024
        }
    }
}
#[derive(Debug, Clone)]
pub struct SiteAssetReference {
    pub slot: &'static str,
    pub asset_id: Uuid,
    pub kind: SiteAssetKind,
}
#[derive(Debug, Clone, Serialize)]
pub struct SiteAssetMetadata {
    pub id: Uuid,
    pub kind: SiteAssetKind,
    pub mime: String,
    pub width: u32,
    pub height: u32,
    pub byte_size: u64,
    pub sha256: String,
    pub preview_url: String,
}
pub struct StoredSiteAsset {
    pub mime: String,
    pub sha256: String,
    pub bytes: Vec<u8>,
}
pub struct NewSiteAsset {
    pub metadata: SiteAssetMetadata,
    pub bytes: Vec<u8>,
    pub uploaded_by: i32,
}
#[derive(Debug, Clone, Serialize)]
pub struct ResolvedAuthBranding {
    pub cover_asset_id: Option<Uuid>,
    pub welcome_title: String,
    pub welcome_description: String,
}
#[derive(Debug, Clone, Serialize)]
pub struct ResolvedSiteBranding {
    pub branding: SiteBranding,
    pub login: ResolvedAuthBranding,
    pub oidc: ResolvedAuthBranding,
}

fn text(value: &str, max: usize, multiline: bool) -> Result<(), IdentityError> {
    if value.chars().count() > max
        || value
            .chars()
            .any(|c| c.is_control() && !(multiline && matches!(c, '\n' | '\r' | '\t')))
    {
        return Err(IdentityError::InvalidInput);
    }
    Ok(())
}
fn link(value: &str) -> Result<(), IdentityError> {
    text(value, 2048, false)?;
    let parsed = url::Url::parse(value).map_err(|_| IdentityError::InvalidInput)?;
    if !matches!(parsed.scheme(), "http" | "https")
        || parsed.host_str().is_none()
        || !parsed.username().is_empty()
        || parsed.password().is_some()
        || value.trim() != value
    {
        return Err(IdentityError::InvalidInput);
    }
    Ok(())
}
pub fn validate_settings(input: &SiteSettingsDocument) -> Result<(), IdentityError> {
    let b = &input.branding;
    if b.site_name.trim().is_empty() {
        return Err(IdentityError::InvalidInput);
    }
    for (value, max, multiline) in [
        (&b.site_name, 80, false),
        (&b.site_description, 300, true),
        (&b.welcome_title, 120, true),
        (&b.welcome_description, 1000, true),
        (&b.footer.copyright_text, 300, true),
        (&b.footer.filing_number, 100, false),
    ] {
        text(value, max, multiline)?;
    }
    if let Some(value) = &input.oidc.welcome_title {
        text(value, 120, true)?;
    }
    if let Some(value) = &input.oidc.welcome_description {
        text(value, 1000, true)?;
    }
    if let Some(value) = &b.footer.filing_url {
        link(value)?;
    }
    if b.footer.links.len() > 10 {
        return Err(IdentityError::InvalidInput);
    }
    for item in &b.footer.links {
        text(&item.label, 80, false)?;
        if item.label.trim().is_empty() {
            return Err(IdentityError::InvalidInput);
        }
        link(&item.url)?;
    }
    Ok(())
}
pub fn resolve_branding(input: &SiteSettingsDocument) -> ResolvedSiteBranding {
    let b = &input.branding;
    ResolvedSiteBranding {
        branding: b.clone(),
        login: ResolvedAuthBranding {
            cover_asset_id: b.cover_asset_id,
            welcome_title: b.welcome_title.clone(),
            welcome_description: b.welcome_description.clone(),
        },
        oidc: ResolvedAuthBranding {
            cover_asset_id: input.oidc.cover_asset_id.or(b.cover_asset_id),
            welcome_title: input
                .oidc
                .welcome_title
                .clone()
                .unwrap_or_else(|| b.welcome_title.clone()),
            welcome_description: input
                .oidc
                .welcome_description
                .clone()
                .unwrap_or_else(|| b.welcome_description.clone()),
        },
    }
}
pub fn referenced_assets(input: &SiteSettingsDocument) -> Vec<SiteAssetReference> {
    [
        ("logo", input.branding.logo_asset_id, SiteAssetKind::Logo),
        (
            "favicon",
            input.branding.favicon_asset_id,
            SiteAssetKind::Favicon,
        ),
        ("cover", input.branding.cover_asset_id, SiteAssetKind::Cover),
        (
            "oidc_cover",
            input.oidc.cover_asset_id,
            SiteAssetKind::Cover,
        ),
    ]
    .into_iter()
    .filter_map(|(slot, id, kind)| {
        id.map(|asset_id| SiteAssetReference {
            slot,
            asset_id,
            kind,
        })
    })
    .collect()
}
pub fn changed_fields(before: &SiteSettingsDocument, after: &SiteSettingsDocument) -> Vec<String> {
    let mut changed = Vec::new();
    macro_rules! compare { ($($field:ident).+) => { if before.$($field).+ != after.$($field).+ { changed.push(stringify!($($field).+).replace(' ', "")); } }; }
    compare!(branding.site_name);
    compare!(branding.site_description);
    compare!(branding.logo_asset_id);
    compare!(branding.favicon_asset_id);
    compare!(branding.cover_asset_id);
    compare!(branding.welcome_title);
    compare!(branding.welcome_description);
    compare!(branding.footer.copyright_text);
    compare!(branding.footer.filing_number);
    compare!(branding.footer.filing_url);
    compare!(branding.footer.links);
    compare!(oidc.cover_asset_id);
    compare!(oidc.welcome_title);
    compare!(oidc.welcome_description);
    changed
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn site_branding_defaults_and_unknown_fields() {
        let defaults = SiteSettingsDocument::defaults().unwrap();
        validate_settings(&defaults).unwrap();
        let mut json = serde_json::to_value(&defaults).unwrap();
        json["unknown"] = true.into();
        assert!(serde_json::from_value::<SiteSettingsDocument>(json).is_err());
    }
    #[test]
    fn site_branding_field_boundaries_and_links() {
        let mut d = SiteSettingsDocument::defaults().unwrap();
        d.branding.site_name = "站".repeat(80);
        assert!(validate_settings(&d).is_ok());
        d.branding.site_name.push('站');
        assert!(validate_settings(&d).is_err());
        d.branding.site_name = "  ".into();
        assert!(validate_settings(&d).is_err());
        d.branding.site_name = "企业".into();
        for bad in [
            "javascript:alert(1)",
            "data:text/html,x",
            "/relative",
            "https://user:pass@example.com",
            "https://",
            " https://example.com",
        ] {
            d.branding.footer.filing_url = Some(bad.into());
            assert!(validate_settings(&d).is_err(), "{bad}");
        }
        d.branding.footer.filing_url = Some("https://example.com".into());
        d.branding.footer.links = vec![
            FooterLink {
                label: "链接".into(),
                url: "https://example.com".into()
            };
            10
        ];
        assert!(validate_settings(&d).is_ok());
        d.branding
            .footer
            .links
            .push(d.branding.footer.links[0].clone());
        assert!(validate_settings(&d).is_err());
    }
    #[test]
    fn site_branding_inheritance_empty_text_and_references() {
        let before = SiteSettingsDocument::defaults().unwrap();
        let mut d = before.clone();
        let id = Uuid::new_v4();
        d.branding.cover_asset_id = Some(id);
        assert_eq!(resolve_branding(&d).oidc.cover_asset_id, Some(id));
        d.oidc.welcome_title = Some(String::new());
        assert_eq!(resolve_branding(&d).oidc.welcome_title, "");
        d.oidc.cover_asset_id = Some(id);
        assert_eq!(referenced_assets(&d).len(), 2);
        assert_eq!(
            changed_fields(&before, &d),
            vec![
                "branding.cover_asset_id",
                "oidc.cover_asset_id",
                "oidc.welcome_title"
            ]
        );
    }
    #[test]
    fn site_branding_all_text_limits_are_unicode_boundaries() {
        type Setter = fn(&mut SiteSettingsDocument, String);
        let fields: &[(&str, usize, Setter)] = &[
            ("name", 80, |d, v| d.branding.site_name = v),
            ("description", 300, |d, v| d.branding.site_description = v),
            ("title", 120, |d, v| d.branding.welcome_title = v),
            ("welcome", 1000, |d, v| d.branding.welcome_description = v),
            ("copyright", 300, |d, v| {
                d.branding.footer.copyright_text = v
            }),
            ("filing", 100, |d, v| d.branding.footer.filing_number = v),
            ("oidc_title", 120, |d, v| d.oidc.welcome_title = Some(v)),
            ("oidc_description", 1000, |d, v| {
                d.oidc.welcome_description = Some(v)
            }),
        ];
        for (name, max, set) in fields {
            let mut d = SiteSettingsDocument::defaults().unwrap();
            set(&mut d, "字".repeat(*max));
            assert!(validate_settings(&d).is_ok(), "{name}");
            set(&mut d, "字".repeat(max + 1));
            assert!(validate_settings(&d).is_err(), "{name}");
        }
        let mut d = SiteSettingsDocument::defaults().unwrap();
        d.branding.footer.links = vec![FooterLink {
            label: "名".repeat(80),
            url: format!("https://example.com/?{}", "a".repeat(2027)),
        }];
        assert_eq!(d.branding.footer.links[0].url.chars().count(), 2048);
        assert!(validate_settings(&d).is_ok());
        d.branding.footer.links[0].url.push('a');
        assert!(validate_settings(&d).is_err());
        d.branding.footer.links[0].url = "https://example.com".into();
        d.branding.footer.links[0].label.push('字');
        assert!(validate_settings(&d).is_err());
        d.branding.footer.links.clear();
        d.branding.welcome_description = "允许\n换行".into();
        assert!(validate_settings(&d).is_ok());
        d.branding.welcome_description.push('\0');
        assert!(validate_settings(&d).is_err());
    }
}
