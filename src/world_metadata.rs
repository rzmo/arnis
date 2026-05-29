//! World `metadata.json` for geographic anchoring and generation settings (append mode).

use crate::args::Args;
use crate::coordinate_system::geographic::LLBBox;
use serde::{Deserialize, Serialize};
use std::path::Path;

/// Geographic point mapped to this world's Minecraft origin (NW corner convention).
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct GeoAnchor {
    pub anchor_lat: f64,
    pub anchor_lng: f64,
}

/// Generation flags stored with the world so append runs can be validated.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct WorldGenerationSettings {
    pub terrain: bool,
    pub fillground: bool,
    pub land_cover: bool,
    pub ground_level: i32,
    pub auto_ground_level: bool,
    pub disable_height_limit: bool,
    pub use_3d: bool,
    pub rotation: f64,
    pub interior: bool,
    pub roof: bool,
    pub aws_only_elevation: bool,
    pub bake_lighting: bool,
    pub arnis_version: String,
}

impl WorldGenerationSettings {
    pub fn from_args(args: &Args) -> Self {
        Self {
            terrain: args.terrain,
            fillground: args.fillground,
            land_cover: args.land_cover,
            ground_level: args.ground_level,
            auto_ground_level: args.auto_ground_level,
            disable_height_limit: args.disable_height_limit,
            use_3d: args.use_3d,
            rotation: args.rotation,
            interior: args.interior,
            roof: args.roof,
            aws_only_elevation: args.aws_only_elevation,
            bake_lighting: args.bake_lighting,
            arnis_version: env!("CARGO_PKG_VERSION").to_string(),
        }
    }
}

/// Metadata saved with the world (`metadata.json` in the world folder).
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct WorldMetadata {
    pub min_mc_x: i32,
    pub max_mc_x: i32,
    pub min_mc_z: i32,
    pub max_mc_z: i32,

    pub min_geo_lat: f64,
    pub max_geo_lat: f64,
    pub min_geo_lon: f64,
    pub max_geo_lon: f64,

    pub scale: f64,
    pub anchor_lat: f64,
    pub anchor_lng: f64,

    #[serde(default)]
    pub settings: Option<WorldGenerationSettings>,
}

impl WorldMetadata {
    /// NW-corner anchor for a bbox (matches `CoordTransformer` local origin).
    pub fn anchor_for_bbox(llbbox: &LLBBox) -> GeoAnchor {
        GeoAnchor {
            anchor_lat: llbbox.max().lat(),
            anchor_lng: llbbox.min().lng(),
        }
    }

    pub fn anchor(&self) -> GeoAnchor {
        GeoAnchor {
            anchor_lat: self.anchor_lat,
            anchor_lng: self.anchor_lng,
        }
    }

    /// Union geographic and Minecraft extents with another area.
    pub fn union_with_area(&mut self, llbbox: &LLBBox, xzbbox: &crate::coordinate_system::cartesian::XZBBox) {
        self.min_geo_lat = self.min_geo_lat.min(llbbox.min().lat());
        self.max_geo_lat = self.max_geo_lat.max(llbbox.max().lat());
        self.min_geo_lon = self.min_geo_lon.min(llbbox.min().lng());
        self.max_geo_lon = self.max_geo_lon.max(llbbox.max().lng());

        self.min_mc_x = self.min_mc_x.min(xzbbox.min_x());
        self.max_mc_x = self.max_mc_x.max(xzbbox.max_x());
        self.min_mc_z = self.min_mc_z.min(xzbbox.min_z());
        self.max_mc_z = self.max_mc_z.max(xzbbox.max_z());
    }

    pub fn build_new(
        llbbox: &LLBBox,
        xzbbox: &crate::coordinate_system::cartesian::XZBBox,
        scale: f64,
        settings: WorldGenerationSettings,
    ) -> Self {
        let anchor = Self::anchor_for_bbox(llbbox);
        Self {
            min_mc_x: xzbbox.min_x(),
            max_mc_x: xzbbox.max_x(),
            min_mc_z: xzbbox.min_z(),
            max_mc_z: xzbbox.max_z(),
            min_geo_lat: llbbox.min().lat(),
            max_geo_lat: llbbox.max().lat(),
            min_geo_lon: llbbox.min().lng(),
            max_geo_lon: llbbox.max().lng(),
            scale,
            anchor_lat: anchor.anchor_lat,
            anchor_lng: anchor.anchor_lng,
            settings: Some(settings),
        }
    }
}

/// Read `metadata.json` from a world directory.
pub fn read_world_metadata(world_dir: &Path) -> Result<WorldMetadata, String> {
    let path = world_dir.join("metadata.json");
    if !path.exists() {
        return Err("metadata.json not found — select an Arnis-generated Java world.".to_string());
    }
    let contents =
        std::fs::read_to_string(&path).map_err(|e| format!("Failed to read metadata.json: {e}"))?;
    serde_json::from_str(&contents).map_err(|e| format!("Failed to parse metadata.json: {e}"))
}

/// Write metadata, unioning with any existing file when appending.
pub fn write_world_metadata(
    world_dir: &Path,
    llbbox: &LLBBox,
    xzbbox: &crate::coordinate_system::cartesian::XZBBox,
    scale: f64,
    settings: WorldGenerationSettings,
    append: bool,
) -> Result<(), String> {
    let path = world_dir.join("metadata.json");
    let mut metadata = if append {
        match read_world_metadata(world_dir) {
            Ok(mut existing) => {
                existing.union_with_area(llbbox, xzbbox);
                if existing.settings.is_none() {
                    existing.settings = Some(settings.clone());
                }
                existing
            }
            Err(_) => WorldMetadata::build_new(llbbox, xzbbox, scale, settings.clone()),
        }
    } else {
        WorldMetadata::build_new(llbbox, xzbbox, scale, settings.clone())
    };

    metadata.scale = scale;
    if metadata.settings.is_none() {
        metadata.settings = Some(settings);
    }

    let contents =
        serde_json::to_string_pretty(&metadata).map_err(|e| format!("Failed to serialize metadata: {e}"))?;
    std::fs::write(&path, contents).map_err(|e| format!("Failed to write metadata.json: {e}"))
}

/// Hard validation before append generation.
pub fn validate_append_hard(
    stored: &WorldMetadata,
    requested_scale: f64,
    requested_rotation: f64,
) -> Result<(), String> {
    if (stored.scale - requested_scale).abs() > 1e-6 {
        return Err(format!(
            "Scale must match the existing world (stored {:.2}, requested {:.2}).",
            stored.scale, requested_scale
        ));
    }
    if requested_rotation.abs() > f64::EPSILON {
        return Err(
            "Rotation must be 0 when adding to an existing world.".to_string(),
        );
    }
    Ok(())
}

/// Soft warnings when append settings differ (coordinate-safe but visually inconsistent).
pub fn append_setting_warnings(
    stored: &WorldGenerationSettings,
    requested: &WorldGenerationSettings,
) -> Vec<String> {
    let mut warnings = Vec::new();
    if stored.terrain != requested.terrain {
        warnings.push("Terrain setting differs from the existing world.".to_string());
    }
    if stored.fillground != requested.fillground {
        warnings.push("Fill ground setting differs from the existing world.".to_string());
    }
    if stored.land_cover != requested.land_cover {
        warnings.push("Land cover setting differs from the existing world.".to_string());
    }
    if stored.ground_level != requested.ground_level
        || stored.auto_ground_level != requested.auto_ground_level
    {
        warnings.push("Ground level settings differ from the existing world.".to_string());
    }
    if stored.disable_height_limit != requested.disable_height_limit {
        warnings.push("Extend build height setting differs from the existing world.".to_string());
    }
    if stored.use_3d != requested.use_3d {
        warnings.push("3D models setting differs from the existing world.".to_string());
    }
    warnings
}

// Manual adjacency test: generate area A as a new Java world, then generate an adjacent
// area B with "Add to existing" into the same folder. In-game, A should remain intact,
// B should align geographically, and shared region seams should have no gaps.

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utilities::get_llbbox_arnis;

    #[test]
    fn metadata_union_expands_bounds() {
        let ll1 = get_llbbox_arnis();
        let (mut meta, xzb1) = {
            let (t, xzb) =
                crate::coordinate_system::transformation::CoordTransformer::llbbox_to_xzbbox(&ll1, 1.0)
                    .unwrap();
            let _ = t;
            (
                WorldMetadata::build_new(&ll1, &xzb, 1.0, WorldGenerationSettings::from_args(&default_args())),
                xzb,
            )
        };

        let ll2 = LLBBox::new(
            ll1.min().lat() - 0.02,
            ll1.max().lng() + 0.01,
            ll1.min().lat() - 0.01,
            ll1.max().lng() + 0.02,
        )
        .unwrap();
        let anchor = meta.anchor();
        let (_, xzb2) =
            crate::coordinate_system::transformation::CoordTransformer::llbbox_to_xzbbox_anchored(
                &ll2, 1.0, anchor.anchor_lat, anchor.anchor_lng,
            )
            .unwrap();

        meta.union_with_area(&ll2, &xzb2);
        assert!(meta.min_mc_x <= xzb1.min_x().min(xzb2.min_x()));
        assert!(meta.max_mc_x >= xzb1.max_x().max(xzb2.max_x()));
    }

    fn default_args() -> crate::args::Args {
        crate::args::Args {
            bbox: get_llbbox_arnis(),
            file: None,
            save_json_file: None,
            path: None,
            append_to_world: None,
            bedrock: false,
            luanti: false,
            downloader: "requests".to_string(),
            scale: 1.0,
            ground_level: -62,
            auto_ground_level: false,
            terrain: false,
            interior: true,
            roof: true,
            fillground: false,
            land_cover: true,
            use_3d: false,
            debug: false,
            timeout: None,
            spawn_lat: None,
            spawn_lng: None,
            rotation: 0.0,
            disable_height_limit: false,
            aws_only_elevation: false,
            benchmark: false,
            bake_lighting: false,
        }
    }
}
