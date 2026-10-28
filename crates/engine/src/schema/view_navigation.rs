//! Render shell requests. The UI applies validated echoes without design edits.
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Deserialize, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RenderDock {
    pub panel: RenderPanel,
    pub side: DockSide,
}

#[derive(Debug, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum RenderPanel {
    Inspector,
    Browser,
}

#[derive(Debug, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum DockSide {
    Left,
    Right,
}

#[derive(Debug, Deserialize, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RenderFocus {
    /// Hide the Render docks while retaining their layout and the current camera.
    pub on: bool,
}

#[derive(Debug, Deserialize, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RenderBrowser {
    /// Open the floating Browser card in a narrow Render window.
    pub open: bool,
}

/// Show or hide the View angles readout under the orientation gizmo (#6032): the view's pitch, yaw
/// and roll in degrees (the angles of `view.apply`'s camera, which takes them in radians) and Ortho
/// or Persp for the projection. A display setting the app keeps: not saved with the design, not an
/// undo step, and it never opens a dialog.
#[derive(Debug, Deserialize, JsonSchema)]
#[schemars(deny_unknown_fields)]
pub struct AngleReadoutParams {
    /// True shows the readout, false hides it.
    #[serde(deserialize_with = "enabled_flag")]
    pub enabled: bool,
}

/// A boolean, with the `view.angle_readout` handler's message for anything else.
fn enabled_flag<'de, D: serde::Deserializer<'de>>(d: D) -> Result<bool, D::Error> {
    match Value::deserialize(d)? {
        Value::Bool(b) => Ok(b),
        _ => Err(serde::de::Error::custom("must be a boolean")),
    }
}

/// What `view.angle_readout` returns.
#[derive(Debug, Serialize, JsonSchema)]
pub struct AngleReadoutResult {
    /// Whether the readout is shown now.
    pub enabled: bool,
    /// In the warn parameter-check mode: what the call's parameters got wrong.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub param_warnings: Vec<String>,
}

#[cfg(test)]
#[path = "view_navigation_tests.rs"]
mod tests;
