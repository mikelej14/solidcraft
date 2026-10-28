use super::*;
use crate::{scenario::Harness, theme::Tokens};
fn fixture() -> Harness {
    let mut h = Harness::new();
    h.call("ui.resize", json!({"width":1440,"height":1000}));
    h.app.run("solid.box", json!({"length":20,"width":15,"height":10,"body_name":"Subject"})).unwrap();
    super::super::enter(&mut h.app, true).unwrap();
    h.app.run("view.render_stage", json!({"stage":"output"})).unwrap();
    h.frames(3);
    h
}
#[test]
fn native_inspector_defaults_to_output_and_keeps_job_actions_on_screen_in_both_shells() {
    for dark in [false, true] {
        for layout in [crate::shell::LayoutMode::Docked, crate::shell::LayoutMode::Floating] {
            let mut h = fixture();
            h.app.preferences.theme = if dark { "dark" } else { "light" }.into();
            h.app.ui.layout = layout;
            h.call("ui.resize", json!({"width":1280,"height":900}));
            assert_eq!(h.app.render_workspace.view.inspector.page, Page::Output);
            let before = h.app.session.doc.to_json();
            let revision = h.app.session.revision;
            let history = (h.app.session.undo.len(), h.app.session.redo.len());
            for page in Page::ALL {
                h.app.render_workspace.view.inspector.page = page;
                h.frames(3);
                let inspector = crate::scenario::rect_of("render:inspector").unwrap();
                if page == Page::Appearance {
                    continue;
                }
                let footer = crate::scenario::rect_of("render:job-footer").unwrap();
                assert!(inspector.contains_rect(footer), "footer outside inspector at {page:?}: {inspector:?} {footer:?}");
                for name in ["render:check", "render:start", "render:cancel", "render:export"] {
                    let target = crate::scenario::rect_of(name).unwrap();
                    assert!(footer.contains_rect(target), "{name} clipped at {page:?}");
                }
                assert_eq!(h.app.session.doc.to_json(), before);
                assert_eq!(h.app.session.revision, revision);
                assert_eq!((h.app.session.undo.len(), h.app.session.redo.len()), history);
                assert!(h.app.render_workspace.view.job.is_none());
                assert!(h.app.render_workspace.view.completed.is_none());
            }
        }
    }
}
#[test]
fn stage_navigation_is_local_and_never_switches_executor_or_overwrites_drafts() {
    let mut h = fixture();
    let before = h.app.session.doc.to_json();
    h.app.render_workspace.view.samples = 37;
    h.app.render_workspace.view.draft.width = 777;
    for stage in super::super::Stage::ALL {
        h.step(&json!({"click":{"handle":format!("render:stage:{}",stage.key())}})).unwrap();
        assert_eq!(h.app.render_workspace.view.inspector.page, stage.page());
        assert_eq!(h.app.render_workspace.view.method, ImageMethod::Raster);
        assert_eq!(h.app.render_workspace.view.samples, 37);
        assert_eq!(h.app.render_workspace.view.draft.width, 777);
        assert_eq!(h.app.session.doc.to_json(), before);
    }
}

#[test]
fn dirty_shot_switch_keep_discard_and_save_are_explicit_and_lossless() {
    let mut h = fixture();
    h.app.render_workspace.view.name = "A".into();
    setups::save(&mut h.app).unwrap();
    h.app.run("render.settings.set", json!({"name":"B","set":{"width":321,"job":{"method":"raster"}}})).unwrap();
    h.app.render_workspace.view.draft.width = 123;
    setups::refresh_dirty(&mut h.app);
    request_action(&mut h.app, PendingAction::Switch("B".into()));
    h.frames(3);
    h.step(&json!({"widget":h.app.tr("render.keep_editing")})).unwrap();
    assert_eq!(h.app.render_workspace.view.name, "A");
    assert_eq!(h.app.render_workspace.view.draft.width, 123);
    request_action(&mut h.app, PendingAction::Switch("B".into()));
    h.frames(3);
    h.step(&json!({"widget":h.app.tr("render.discard_continue")})).unwrap();
    assert_eq!(h.app.render_workspace.view.name, "B");
    assert_eq!(h.app.render_workspace.view.draft.width, 321);
    assert_ne!(h.app.session.doc.render.setups["A"].width, 123);
    h.app.render_workspace.view.draft.width = 456;
    setups::refresh_dirty(&mut h.app);
    request_action(&mut h.app, PendingAction::Switch("A".into()));
    h.frames(3);
    h.step(&json!({"widget":h.app.tr("render.save_continue")})).unwrap();
    assert_eq!(h.app.session.doc.render.setups["B"].width, 456);
    assert_eq!(h.app.render_workspace.view.name, "A");
}
#[test]
fn expired_confirmation_cannot_remove_a_changed_shot() {
    let mut h = fixture();
    setups::save(&mut h.app).unwrap();
    let name = h.app.render_workspace.view.name.clone();
    request_action(&mut h.app, PendingAction::Remove(name.clone()));
    h.app.run("solid.box", json!({"length":1,"width":1,"height":1,"body_name":"Later"})).unwrap();
    h.frames(3);
    assert!(h.app.render_workspace.view.inspector.pending.is_none());
    assert!(h.app.session.doc.render.setups.contains_key(&name));
}
#[test]
fn projected_quality_changes_only_requested_budget_fields_and_has_no_runtime_claim() {
    let mut h = fixture();
    super::super::select_image_method(&mut h.app, ImageMethod::GpuRaytrace);
    h.app.cam.fov = 0.6;
    h.app.render_workspace.view.studio.intensity = 0.42;
    h.app.render_workspace.view.draft.exposure_ev = 1.23;
    h.app.render_workspace.view.studio.ambient = [0.11, 0.22, 0.33];
    let before = h.app.session.doc.to_json();
    let camera = h.app.cam;
    let plan = h.app.run("render.quality.prepare", json!({"id":"Q-002","variant":"fixed_samples_png","acknowledge_omissions":true})).unwrap();
    apply_quality(&mut h.app, &plan).unwrap();
    assert_eq!((h.app.render_workspace.view.draft.width, h.app.render_workspace.view.draft.height), (1600, 900));
    assert_eq!(h.app.render_workspace.view.samples, 512);
    assert_eq!(h.app.cam, camera);
    assert_eq!(h.app.render_workspace.view.studio.intensity, 0.42);
    assert_eq!(h.app.render_workspace.view.studio.ambient, [0.11, 0.22, 0.33]);
    assert_eq!(h.app.render_workspace.view.draft.exposure_ev, 1.23);
    assert_eq!(h.app.session.doc.to_json(), before);
    super::super::select_image_method(&mut h.app, ImageMethod::CpuRaytrace);
    assert!(apply_quality(&mut h.app, &plan).is_err());
}
#[test]
fn complete_native_studio_can_be_reviewed_and_applied_from_real_ui_controls() {
    let mut h = fixture();
    super::super::select_image_method(&mut h.app, ImageMethod::GpuRaytrace);
    setups::save(&mut h.app).unwrap();
    library::open_category(&mut h.app, "native_scenes");
    h.frames(4);
    h.step(&json!({"click":{"handle":"render:library:NATIVE-STUDIO-002"}})).unwrap();
    let before = h.app.session.doc.to_json();
    h.step(&json!({"widget":h.app.tr("render.scene_review")})).unwrap();
    assert_eq!(h.app.session.doc.to_json(), before);
    assert!(h.app.render_workspace.view.inspector.scene_review.is_some());
    h.step(&json!({"widget":h.app.tr("render.scene_consent")})).unwrap();
    h.step(&json!({"widget":h.app.tr("render.scene_apply")})).unwrap();
    assert!(!h.app.render_workspace.view.draft.stage.is_empty());
    assert!(h.app.render_workspace.view.inspector.scene_review.is_none());
    assert!(!h.app.render_workspace.view.dirty);
}
#[test]
fn unsupported_output_is_visible_and_never_fakes_export_or_clamps_intent() {
    let mut h = fixture();
    super::super::select_image_method(&mut h.app, ImageMethod::GpuRaytrace);
    h.app.render_workspace.view.inspector.page = Page::Output;
    h.app.render_workspace.view.draft.width = 7000;
    h.app.render_workspace.view.draft.height = 6000;
    h.app.render_workspace.view.samples = 37;
    h.frames(4);
    assert_eq!((h.app.render_workspace.view.draft.width, h.app.render_workspace.view.draft.height), (7000, 6000));
    assert_eq!(h.app.render_workspace.view.samples, 37);
    assert!(super::super::image_request(&h.app).is_err());
    assert!(h.app.render_workspace.view.completed.is_none());
    assert!(h.app.tr("render.output_format_status").contains("EXR"));
}

#[test]
fn rational_resolution_lock_preserves_anchor_rounding_and_rejects_without_partial_changes() {
    let mut edit = Resolution { locked: true, ..Default::default() };
    let mut size = [3840, 2160];
    size = edit.edit(size, 0, 1920, 8192).unwrap();
    assert_eq!(size, [1920, 1080]);
    size = edit.edit(size, 1, 720, 8192).unwrap();
    assert_eq!(size, [1280, 720]);
    let anchor = edit.anchor;
    for _ in 0..100 {
        size = edit.edit(size, 0, 1001, 8192).unwrap();
        assert_eq!(size, [1001, 563]);
        size = edit.edit(size, 0, 1920, 8192).unwrap();
        assert_eq!(size, [1920, 1080]);
    }
    assert_eq!(edit.anchor, anchor, "rounded edits must not drift the anchor");
    assert!(edit.edit(size, 1, 8192, 8192).is_none());
    assert_eq!(edit.observed, Some(size), "a refused linked dimension must not partly change size");
    assert!(edit.rejected);
    assert!(edit.edit(size, 0, 0, 8192).is_none());
    assert!(edit.edit(size, 2, 100, 8192).is_none());
    let mut extreme = Resolution { locked: true, ..Default::default() };
    assert!(extreme.edit([1, u32::MAX], 0, u32::MAX, u32::MAX).is_none());
    assert!(Resolution { locked: true, ..Default::default() }.edit([0, 1], 0, 1, 8192).is_none());
}

#[test]
fn resolution_lock_reanchors_for_explicit_presets_and_does_not_affect_other_intent() {
    let mut edit = Resolution { locked: true, ..Default::default() };
    assert_eq!(edit.edit([3840, 2160], 0, 1920, 8192), Some([1920, 1080]));
    edit.observe([1080, 1920]);
    assert_eq!(edit.edit([1080, 1920], 1, 960, 8192), Some([540, 960]));
    edit.observe([1000, 1000]);
    assert_eq!(edit.edit([1000, 1000], 0, 777, 8192), Some([777, 777]));
    edit.locked = false;
    assert_eq!(edit.edit([777, 777], 0, 555, 8192), Some([555, 777]));
    edit.locked = true;
    assert_eq!(edit.edit([555, 777], 0, 1110, 8192), Some([1110, 1554]));
}

#[test]
fn actual_output_ratio_and_orientation_controls_are_local_and_preserve_saved_capture() {
    let mut h = fixture();
    super::super::select_image_method(&mut h.app, ImageMethod::GpuRaytrace);
    h.app.render_workspace.view.draft.width = 1920;
    h.app.render_workspace.view.draft.height = 1080;
    h.app.render_workspace.view.samples = 37;
    h.app.render_workspace.view.inspector.page = Page::Output;
    setups::save(&mut h.app).unwrap();
    let saved = h.app.session.doc.to_json();
    let camera = h.app.cam;
    h.frames(3);
    h.step(&json!({"click":{"handle":"render:aspect-lock"}})).unwrap();
    assert!(h.app.render_workspace.view.inspector.resolution.locked);
    assert!(!h.app.render_workspace.view.dirty, "coupling editor coordinates is not a saved numerical edit");
    h.step(&json!({"click":{"handle":"render:swap-size"}})).unwrap();
    assert_eq!((h.app.render_workspace.view.draft.width, h.app.render_workspace.view.draft.height), (1080, 1920));
    assert_eq!(h.app.render_workspace.view.samples, 37);
    assert_eq!(h.app.cam, camera);
    assert_eq!(h.app.session.doc.to_json(), saved);
    assert!(h.app.render_workspace.view.dirty);
    setups::load(&mut h.app);
    h.frames(3);
    assert_eq!(h.app.render_workspace.view.inspector.resolution.anchor, Some([1920, 1080]));
}

#[test]
fn contextual_scope_notes_wrap_with_native_tokens_instead_of_clipping() {
    let mut h = fixture();
    h.app.render_workspace.view.inspector.page = Page::Scene;
    h.frames(3);
    let rect = crate::scenario::rect_of("dialog:note:render.scene_preserves_design").unwrap();
    let inspector = crate::scenario::rect_of("render:inspector").unwrap();
    let footer = crate::scenario::rect_of("render:job-footer").unwrap();
    assert!(rect.height() > Tokens::get().font_small + Tokens::get().space_8, "long scope notice must have more than one line");
    assert!(inspector.contains_rect(rect));
    assert!(rect.bottom() < footer.top());
}

#[test]
fn temporary_library_maps_preserve_the_reviewed_plane_scale_and_optical_exclusivity() {
    let mut h = fixture();
    super::super::select_image_method(&mut h.app, ImageMethod::GpuRaytrace);
    let targets = h.app.run("render.scene", json!({})).unwrap();
    let target: materials::Target = serde_json::from_value(targets["bodies"][0].clone()).unwrap();
    let before = h.app.session.doc.to_json();
    materials::install_library_surface(&mut h.app, "SURF-001", target.clone(), "yz", 2.5).unwrap();
    let binding = &h.app.render_workspace.view.materials.bindings[0];
    assert_eq!(binding.maps.frame.u, [0.0, 250.0, 0.0]);
    assert_eq!(binding.maps.frame.v, [0.0, 0.0, 250.0]);
    assert_eq!(h.app.session.doc.to_json(), before, "temporary maps must not change the native model");
    let original = binding.maps.frame;
    assert!(materials::install_library_surface(&mut h.app, "SURF-001", target.clone(), "unknown", 1.0).is_err());
    assert!(materials::install_library_surface(&mut h.app, "SURF-001", target.clone(), "xy", 0.0).is_err());
    assert_eq!(h.app.render_workspace.view.materials.bindings[0].maps.frame, original);
    h.app.render_workspace.view.materials.bindings[0].glass_enabled = true;
    assert!(materials::install_library_surface(&mut h.app, "SURF-001", target, "xy", 1.0).is_err());
    assert!(h.app.render_workspace.view.materials.bindings[0].glass_enabled);
    assert_eq!(h.app.session.doc.to_json(), before);
}

#[test]
fn native_composition_review_actions_are_visible_before_auxiliary_resource_details() {
    for dark in [false, true] {
        let mut h = fixture();
        h.call("ui.set", json!({"dark":dark}));
        super::super::select_image_method(&mut h.app, ImageMethod::GpuRaytrace);
        setups::save(&mut h.app).unwrap();
        library::open_category(&mut h.app, "native_scenes");
        h.frames(3);
        h.step(&json!({"click":{"handle":"render:library:NATIVE-STUDIO-002"}})).unwrap();
        let footer = crate::scenario::rect_of("render:job-footer").unwrap();
        let resolve = crate::scenario::rect_of("render:scene-resolve").unwrap();
        assert!(resolve.bottom() < footer.top(), "the review action must precede supplementary prose");
        h.step(&json!({"click":{"handle":"render:scene-resolve"}})).unwrap();
        h.frames(3);
        for name in ["render:scene-consent", "render:scene-apply"] {
            let rect = crate::scenario::rect_of(name).unwrap();
            assert!(rect.bottom() < footer.top(), "{name} must remain above the pinned job actions: rect={rect:?}, footer={footer:?}, dark={dark}");
        }
    }
}

#[test]
fn ribbon_navigation_reuses_inspector_and_preserves_exact_unsaved_intent() {
    let mut h = fixture();
    h.app.render_workspace.view.samples = 37;
    h.app.render_workspace.view.draft.width = 777;
    let doc = h.app.session.doc.to_json();
    let history = (h.app.session.revision, h.app.session.undo.len(), h.app.session.redo.len());
    for (id, page) in [
        ("render.preset.resolve", Page::Scene),
        ("render.settings.get", Page::Scene),
        ("render.library.appearance.apply", Page::Appearance),
        ("render.library.surface.bind", Page::Appearance),
        ("render.library.studio.prepare", Page::Lighting),
        ("render.library.studio.apply", Page::Lighting),
        ("render.library.environment.apply", Page::Environment),
        ("render.stage.backdrop", Page::Background),
        ("render.quality.prepare", Page::Output),
        ("render.status", Page::Diagnostics),
        ("render.camera.get", Page::Camera),
        ("render.camera.set", Page::Camera),
        ("render.preset.apply", Page::Scene),
        ("render.shot.duplicate", Page::Scene),
        ("render.shot.rename", Page::Scene),
        ("render.shot.remove", Page::Scene),
        ("render.optics.bind", Page::Appearance),
        ("render.optics.unbind", Page::Appearance),
        ("render.library.surface.prepare", Page::Appearance),
        ("render.library.surface.unbind", Page::Appearance),
        ("render.library.physical.prepare", Page::Appearance),
        ("render.stage.ground", Page::Background),
        ("render.stage.clear", Page::Background),
        ("render.hdr.inspect", Page::Environment),
        ("render.settings.capture_hdr", Page::Environment),
        ("render.aging.describe", Page::Appearance),
        ("render.effects.describe", Page::Effects),
        ("render.backends", Page::Diagnostics),
    ] {
        h.app.start(id);
        h.frames(3);
        assert_eq!(h.app.render_workspace.view.inspector.page, page, "{id}");
        assert_eq!(h.app.render_workspace.view.samples, 37);
        assert_eq!(h.app.render_workspace.view.draft.width, 777);
        assert_eq!(h.app.session.doc.to_json(), doc, "navigation may not execute a parameterless apply");
        assert_eq!((h.app.session.revision, h.app.session.undo.len(), h.app.session.redo.len()), history);
        assert!(h.app.render_workspace.view.job.is_none());
        assert!(h.app.render_workspace.view.completed.is_none());
        assert!(!crate::icon_svg::resolve(solidcraft_engine::find_command(id).unwrap().icon).placeholder);
    }
}

#[test]
fn render_stage_bar_keeps_actions_and_personal_preferences_in_both_layouts() {
    assert!(crate::workspace::panels("RENDER").is_empty());
    for layout in [crate::shell::LayoutMode::Docked, crate::shell::LayoutMode::Floating] {
        let mut h = fixture();
        h.app.ui.layout = layout;
        h.app.toolbar_custom.panels.insert("DESIGN/RENDER/RESULT".into(), vec!["render.export".into()]);
        let preferences = h.app.toolbar_custom.prefs();
        for width in [800, 1024, 1440, 2000] {
            h.call("ui.resize", json!({"width":width,"height":900}));
            h.frames(3);
            let bar = crate::scenario::rect_of("render:stage_bar").unwrap();
            let workspace = crate::scenario::rect_of("ribbon:workspace").unwrap();
            assert_eq!(bar.height(), workspace.height());
            assert_eq!(bar.top(), workspace.top());
            for id in ["render:stage:render.start", "render:stage:render.cancel"] {
                if let Some(action) = crate::scenario::rect_of(id) {
                    assert!(bar.contains_rect(action));
                }
            }
            assert!(crate::scenario::rect_of("ribbon:card").is_none());
            assert!(super::super::job_ready(&h.app, "render.cancel").is_err());
            assert!(h.app.render_workspace.view.job.is_none());
            assert_eq!(h.app.toolbar_custom.prefs(), preferences);
        }
    }
}

#[test]
fn ribbon_and_sticky_footer_submit_the_current_draft_not_defaults_or_saved_shot() {
    let mut h = fixture();
    h.app.render_workspace.view.draft.width = 32;
    h.app.render_workspace.view.draft.height = 24;
    h.app.render_workspace.view.draft.background = [24, 63, 107];
    let before = h.app.session.doc.to_json();
    let export = solidcraft_engine::find_command("render.export").unwrap();
    assert!(!super::super::interactive_command_info(&h.app, export).enabled);
    assert!(export.info(&h.app.session).enabled, "UI state must not alter direct engine API semantics");
    h.app.start("render.preflight");
    let request = super::super::image_request(&h.app).unwrap();
    h.step(&json!({"click":{"handle":"render:stage:render.start"}})).unwrap();
    let job = h.app.render_workspace.view.job.expect("ribbon submitted an actual job");
    let handle = h.app.session.render_job_handle(job).unwrap();
    assert_eq!((handle.status().unwrap().request.width, handle.status().unwrap().request.height), (32, 24));
    assert_eq!(handle.status().unwrap().request.background, [24, 63, 107]);
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    while h.app.render_workspace.view.completed != Some(job) {
        assert!(std::time::Instant::now() < deadline, "CPU fixture did not complete");
        h.frames(1);
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
    assert_eq!(super::super::image_request(&h.app).unwrap(), request);
    assert_eq!(h.app.session.doc.to_json(), before);
    assert!(super::super::interactive_command_info(&h.app, export).enabled);
    h.app.render_workspace.view.studio.export_exr = true;
    assert!(!super::super::interactive_command_info(&h.app, export).enabled, "a PNG-only result never pretends to have EXR");
}

#[test]
fn saved_bloom_intent_roundtrips_and_rejects_unsupported_transparent_output() {
    use solidcraft_engine::render::job::bloom::Bloom;
    let mut h = fixture();
    super::super::select_image_method(&mut h.app, ImageMethod::GpuRaytrace);
    h.app.render_workspace.view.studio.finishing.bloom = Some(Bloom { strength: 0.25, threshold: 2.5, radius: 12 });
    let expected = setups::snapshot(&h.app).unwrap();
    setups::save(&mut h.app).unwrap();
    h.app.render_workspace.view.studio.finishing.bloom = None;
    setups::load(&mut h.app);
    assert_eq!(setups::snapshot(&h.app).unwrap(), expected);
    let reopened = solidcraft_engine::doc::Document::from_json(&h.app.session.doc.to_json()).unwrap();
    assert_eq!(reopened.render.setups[&h.app.render_workspace.view.name], expected);
    h.app.render_workspace.view.transparent = true;
    assert!(setups::snapshot(&h.app).is_err());
    h.app.render_workspace.view.transparent = false;
    super::super::select_image_method(&mut h.app, ImageMethod::CpuRaytrace);
    assert!(super::super::image_request(&h.app).is_ok(), "inactive GPU settings remain portable but are not sent to CPU");
}

#[test]
fn ribbon_frame_preserves_explicit_roll_world_up_and_saved_view_authority() {
    let mut h = fixture();
    h.app.cam.roll = 0.37;
    h.app.cam.yaw = 0.21;
    h.app.cam.pitch = 0.43;
    h.app.cam.up = solidcraft_engine::render::UpAxis::Y;
    let orientation = (h.app.cam.roll, h.app.cam.yaw, h.app.cam.pitch, h.app.cam.up);
    let document = h.app.session.doc.to_json();
    let undo = h.app.session.undo.len();
    h.app.start("render.camera.fit");
    assert_eq!((h.app.cam.roll, h.app.cam.yaw, h.app.cam.pitch, h.app.cam.up), orientation);
    assert_eq!(h.app.session.doc.to_json(), document);
    assert_eq!(h.app.session.undo.len(), undo);
    assert!(h.app.session.camera.is_none(), "the explicit view request was consumed by the viewport");
    assert_eq!(h.app.ui.up_axis, h.app.cam.up);
}

#[test]
fn independent_sampling_and_volume_intent_roundtrip_offline_without_silent_conflict_repairs() {
    use solidcraft_engine::render::job::{adaptive::AdaptiveSampling, volume::HomogeneousVolume};
    let mut h = fixture();
    super::super::select_image_method(&mut h.app, ImageMethod::GpuRaytrace);
    h.app.render_workspace.view.samples = 64;
    h.app.render_workspace.view.studio.adaptive =
        Some(AdaptiveSampling { pilot_samples: 16, min_samples: 32, relative_error: 0.03, absolute_error: 0.004, coverage_error: 0.02 });
    h.app.render_workspace.view.studio.volume = Some(HomogeneousVolume {
        min_mm: [-30.0, -20.0, -1.0],
        max_mm: [30.0, 20.0, 40.0],
        extinction_per_mm: 0.002,
        scattering_albedo: [0.7, 0.8, 0.9],
        anisotropy: 0.35,
    });
    h.app.render_workspace.view.studio.linear_exr = true;
    let expected = setups::snapshot(&h.app).unwrap();
    setups::save(&mut h.app).unwrap();
    h.app.render_workspace.view.studio.adaptive = None;
    h.app.render_workspace.view.studio.volume = None;
    setups::load(&mut h.app);
    assert_eq!(setups::snapshot(&h.app).unwrap(), expected);
    let reopened = solidcraft_engine::doc::Document::from_json(&h.app.session.doc.to_json()).unwrap();
    assert_eq!(reopened.render.setups[&h.app.render_workspace.view.name], expected);
    let original = h.app.session.doc.to_json();
    h.app.render_workspace.view.studio.denoise = true;
    assert!(setups::snapshot(&h.app).is_err());
    assert_eq!(h.app.session.doc.to_json(), original);
    h.app.render_workspace.view.studio.denoise = false;
    h.app.render_workspace.view.studio.transmission_samples = 4;
    assert!(setups::snapshot(&h.app).is_err(), "active volume does not silently replace transmission strata");
    h.app.render_workspace.view.studio.volume = None;
    assert!(setups::snapshot(&h.app).is_ok());
    h.app.render_workspace.view.samples = 16;
    assert!(setups::snapshot(&h.app).is_err(), "minimum production count may not be silently clamped to the ceiling");
    h.app.render_workspace.view.samples = 64;
    setups::save(&mut h.app).unwrap();
    let stored = setups::snapshot(&h.app).unwrap();
    h.app.render_workspace.view.studio.transmission_samples = 1;
    setups::load(&mut h.app);
    assert_eq!(setups::snapshot(&h.app).unwrap(), stored);
}

#[test]
fn master_regrade_requires_a_retained_master_instead_of_reconstructing_a_png() {
    let mut h = fixture();
    let spec = solidcraft_engine::find_command("render.result.regrade").unwrap();
    assert!(spec.info(&h.app.session).enabled, "direct API is validated by exact explicit source identity");
    assert!(!super::super::interactive_command_info(&h.app, spec).enabled);
    let document = h.app.session.doc.to_json();
    h.app.start("render.result.regrade");
    assert!(h.app.render_workspace.view.job.is_none());
    assert_eq!(h.app.session.doc.to_json(), document);
    assert!(!crate::icon_svg::resolve(spec.icon).placeholder);
    h.app.render_workspace.view.draft.width = 8;
    h.app.render_workspace.view.draft.height = 8;
    h.app.start("render.start");
    let job = h.app.render_workspace.view.job.unwrap();
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    while h.app.render_workspace.view.completed != Some(job) {
        assert!(std::time::Instant::now() < deadline);
        h.frames(1);
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
    assert!(!super::super::interactive_command_info(&h.app, spec).enabled);
    assert_eq!(h.app.session.doc.to_json(), document);
}

#[test]
fn public_render_inspection_exposes_exact_identities_without_execution_or_document_mutation() {
    let mut h = fixture();
    let doc = h.app.session.doc.to_json();
    let undo = h.app.session.undo.len();
    let camera = h.app.cam;
    let inspected = h.call("ui.inspect", json!({}));
    assert_eq!(inspected["result"]["render"]["active"], true);
    assert_eq!(inspected["result"]["render"]["method"], "raster");
    assert!(inspected["result"]["render"]["job"].is_null());
    h.app.render_workspace.view.draft.width = 8;
    h.app.render_workspace.view.draft.height = 8;
    h.app.start("render.start");
    let identity = h.app.render_workspace.view.job.unwrap();
    let inspected = h.call("ui.inspect", json!({}));
    assert_eq!(inspected["result"]["render"]["job"], json!(identity));
    assert_eq!(h.app.session.doc.to_json(), doc);
    assert_eq!(h.app.session.undo.len(), undo);
    assert_eq!(h.app.cam, camera);
    h.app.session.execute("render.cancel", &json!({"job":identity})).unwrap();
}

#[test]
fn aging_tab_adds_reorders_disables_and_persists_layers_without_engineering_edits() {
    use solidcraft_engine::render::job::aging::AgingKind;
    let mut h = fixture();
    super::super::select_image_method(&mut h.app, ImageMethod::GpuRaytrace);
    let before = h.app.session.doc.to_json();
    h.app.start("render.aging.describe");
    h.frames(3);
    assert_eq!(h.app.render_workspace.view.inspector.page, Page::Appearance);
    assert!(!crate::icon_svg::resolve(solidcraft_engine::find_command("render.aging.describe").unwrap().icon).placeholder);
    for code in [1, 3] {
        h.step(&json!({"click":{"handle":"render:aging-add"}})).unwrap();
        h.frames(2);
        h.step(&json!({"click":{"handle":format!("dialog:list:aging-add-{code}")}})).unwrap();
        h.frames(2);
    }
    assert_eq!(h.app.render_workspace.view.studio.aging.len(), 2);
    assert_eq!(h.app.render_workspace.view.studio.aging.get(1).unwrap().kind, AgingKind::Dust);
    h.step(&json!({"click":{"handle":"render:aging-up"}})).unwrap();
    h.frames(2);
    assert_eq!(h.app.render_workspace.view.studio.aging.get(0).unwrap().kind, AgingKind::Dust);
    h.step(&json!({"click":{"handle":"render:aging-enabled"}})).unwrap();
    h.frames(2);
    assert!(!h.app.render_workspace.view.studio.aging.get(0).unwrap().enabled);
    assert_eq!(h.app.session.doc.to_json(), before, "editing layers only edits the render draft");
    let targets = super::super::materials::read_targets(&mut h.app).unwrap();
    let identity = targets.iter().find(|target| target.name == "Subject").unwrap().stable_target.clone().unwrap();
    h.app.render_workspace.view.studio.aging.get_mut(0).unwrap().target = Some(identity.clone());
    let expected = setups::snapshot(&h.app).unwrap();
    let wire = serde_json::to_value(&expected).unwrap();
    assert_eq!(wire["job"]["options"]["aging"][0]["target"]["definition"], "Subject");
    assert!(wire["job"]["options"]["aging"][0]["target"].get("body_index").is_none());
    setups::save(&mut h.app).unwrap();
    h.app.render_workspace.view.studio.aging = Default::default();
    setups::load(&mut h.app);
    assert_eq!(setups::snapshot(&h.app).unwrap(), expected);
    let reopened = solidcraft_engine::doc::Document::from_json(&h.app.session.doc.to_json()).unwrap();
    assert_eq!(reopened.render.setups[&h.app.render_workspace.view.name], expected);
    assert_eq!(h.app.render_workspace.view.studio.aging.get(0).unwrap().target.as_ref(), Some(&identity));
    h.step(&json!({"click":{"handle":"render:aging-remove"}})).unwrap();
    h.frames(2);
    assert_eq!(h.app.render_workspace.view.studio.aging.len(), 1);
}

#[test]
fn effects_tab_retains_ordered_looks_and_photographic_intent_across_save_and_reload() {
    use solidcraft_engine::render::job::looks::LookKind;
    let mut h = fixture();
    super::super::select_image_method(&mut h.app, ImageMethod::GpuRaytrace);
    let before = h.app.session.doc.to_json();
    h.app.start("render.effects.describe");
    h.frames(3);
    assert_eq!(h.app.render_workspace.view.inspector.page, Page::Effects);
    for code in [3, 6] {
        h.step(&json!({"click":{"handle":"render:look-add"}})).unwrap();
        h.frames(2);
        h.step(&json!({"click":{"handle":format!("dialog:list:look-add-{code}")}})).unwrap();
        h.frames(2);
    }
    assert_eq!(h.app.render_workspace.view.studio.finishing.looks.len(), 2);
    assert_eq!(h.app.render_workspace.view.studio.finishing.looks.get(0).unwrap().kind, LookKind::Warm);
    h.step(&json!({"click":{"handle":"render:look-up"}})).unwrap();
    h.frames(2);
    assert_eq!(h.app.render_workspace.view.studio.finishing.looks.get(0).unwrap().kind, LookKind::Faded);
    assert_eq!(h.app.session.doc.to_json(), before);
    h.app.render_workspace.view.studio.finishing.photographic.chromatic_pixels = 1.5;
    h.app.render_workspace.view.studio.finishing.photographic.flare.strength = 0.4;
    let expected = setups::snapshot(&h.app).unwrap();
    setups::save(&mut h.app).unwrap();
    h.app.render_workspace.view.studio.finishing = Default::default();
    setups::load(&mut h.app);
    assert_eq!(setups::snapshot(&h.app).unwrap(), expected);
    let doc = h.app.session.doc.to_json();
    h.app.render_workspace.view.transparent = true;
    assert!(super::super::ribbon::presentation(&h.app).validate().is_err(), "spatial lens effects cannot silently flatten alpha");
    assert_eq!(h.app.session.doc.to_json(), doc);
}

#[test]
fn backgrounds_ribbon_exposes_camera_and_physical_options_without_mutating_draft_or_switching_renderer() {
    let mut h = fixture();
    super::super::select_image_method(&mut h.app, ImageMethod::GpuRaytrace);
    h.app.render_workspace.view.studio.backdrop_enabled = true;
    h.app.render_workspace.view.studio.backdrop.first_srgb = [30, 60, 90];
    h.app.render_workspace.view.dirty = true;
    let draft = setups::snapshot(&h.app).unwrap();
    let doc = h.app.session.doc.to_json();
    let revision = h.app.session.revision;
    h.app.start("render.stage.backdrop");
    h.frames(3);
    assert_eq!(h.app.render_workspace.view.inspector.page, Page::Background);
    assert_eq!(setups::snapshot(&h.app).unwrap(), draft);
    assert_eq!(h.app.session.doc.to_json(), doc);
    assert_eq!(h.app.session.revision, revision);
    assert!(crate::scenario::rect_of("render:background-color").is_some());
    assert!(crate::scenario::rect_of("render:backdrop-enabled").is_some());
    assert!(crate::scenario::rect_of("render:background-regrade").is_some());
    assert!(h.app.render_workspace.view.job.is_none());
    assert!(
        super::super::ribbon::action_ready(&h.app, "render.result.regrade").is_err(),
        "no raw master cannot be silently reconstructed from the viewport"
    );
}

#[test]
fn clearing_physical_stage_preserves_unsaved_camera_background_and_output_intent() {
    let mut h = fixture();
    h.call("ui.resize", json!({"width":1600,"height":2600}));
    super::super::select_image_method(&mut h.app, ImageMethod::GpuRaytrace);
    setups::save(&mut h.app).unwrap();
    let name = h.app.render_workspace.view.name.clone();
    h.app.run("render.stage.ground", json!({"name":name})).unwrap();
    setups::load(&mut h.app);
    assert_eq!(h.app.render_workspace.view.draft.stage.len(), 1);
    h.app.render_workspace.view.draft.background = [17, 53, 89];
    h.app.render_workspace.view.draft.width = 421;
    h.app.render_workspace.view.draft.height = 239;
    h.app.render_workspace.view.draft.exposure_ev = 1.25;
    h.app.render_workspace.view.studio.backdrop_enabled = true;
    h.app.render_workspace.view.studio.backdrop.mode = solidcraft_engine::render::job::finishing::BackdropMode::Radial;
    h.app.render_workspace.view.studio.backdrop.center = [0.3, 0.6];
    h.app.render_workspace.view.dirty = true;
    let mut expected = setups::snapshot(&h.app).unwrap();
    expected.stage = Default::default();
    let geometry = h.app.session.doc.features.clone();
    let materials = h.app.session.doc.materials.clone();
    h.app.start("render.stage.clear");
    h.frames(4);
    assert_eq!(h.app.render_workspace.view.inspector.page, Page::Background);
    h.step(&json!({"click":{"handle":"render:stage-clear"}})).unwrap();
    h.frames(3);
    assert_eq!(setups::snapshot(&h.app).unwrap(), expected, "clearing stage must keep the unsaved numerical draft, not reload an older one");
    assert_eq!(h.app.session.doc.render.setups[&name], expected);
    assert_eq!(h.app.session.doc.features, geometry);
    assert_eq!(h.app.session.doc.materials, materials);
    let reopened = solidcraft_engine::doc::Document::from_json(&h.app.session.doc.to_json()).unwrap();
    assert_eq!(reopened.render.setups[&name], expected);
}

#[test]
fn render_camera_labels_do_not_shadow_mechanical_pitch_and_axes_remain_explicit() {
    let en = crate::i18n::Locale::for_language("en");
    let de = crate::i18n::Locale::for_language("de");
    assert_eq!(de.label("Pitch"), "Steigung");
    assert_eq!(de.text("coupling.pitch"), "Steigung");
    assert_eq!(en.text("render.camera_pitch"), "Camera pitch");
    assert_ne!(en.text("render.camera_pitch"), en.text("coupling.pitch"));
    assert_eq!(de.text("render.camera_pitch"), "Kameraneigung");
    for locale in ["en", "de", "es", "ja", "zh"] {
        let locale = crate::i18n::Locale::for_language(locale);
        assert_eq!(locale.text("material.editor.axes_numeric"), "1 / 2 / 3");
        assert_eq!(locale.text("material.editor.axes_wood"), "L / R / T");
    }
}

#[test]
fn camera_draft_uses_render_command_and_keeps_saved_design_unchanged() {
    let mut h = fixture();
    let document = h.app.session.doc.to_json();
    let undo = h.app.session.undo.len();
    for up in [solidcraft_engine::render::UpAxis::Z, solidcraft_engine::render::UpAxis::Y] {
        let mut camera = h.app.cam;
        camera.up = up;
        camera.roll = 0.41;
        camera.pitch = 0.27;
        apply_camera_draft(&mut h.app, camera).unwrap();
        assert_eq!(h.app.cam, camera);
        assert!(h.app.render_workspace.view.dirty);
        let mut invalid = camera;
        invalid.distance = 0.0;
        assert!(apply_camera_draft(&mut h.app, invalid).is_err());
        assert_eq!(h.app.cam, camera);
        fit_subject(&mut h.app).unwrap();
        assert_eq!((h.app.cam.up, h.app.cam.roll, h.app.cam.pitch), (up, camera.roll, camera.pitch));
        assert_eq!(h.app.session.doc.to_json(), document);
        assert_eq!(h.app.session.undo.len(), undo);
    }
}

#[test]
fn shots_strip_uses_the_existing_dirty_shot_confirmation() {
    let mut h = fixture();
    h.app.render_workspace.view.name = "A".into();
    setups::save(&mut h.app).unwrap();
    h.app.run("render.settings.set", json!({"name":"B","set":{"width":321,"job":{"method":"raster"}}})).unwrap();
    h.app.render_workspace.view.draft.width = 123;
    h.frames(3);
    h.step(&json!({"click":{"handle":"render:shots:setup:B"}})).unwrap();
    assert!(confirmation_open(&h.app));
    assert_eq!(h.app.render_workspace.view.name, "A");
    h.step(&json!({"widget":h.app.tr("render.keep_editing")})).unwrap();
    assert_eq!(h.app.render_workspace.view.draft.width, 123);
    h.step(&json!({"click":{"handle":"render:shots:setup:B"}})).unwrap();
    h.step(&json!({"widget":h.app.tr("render.discard_continue")})).unwrap();
    assert_eq!(h.app.render_workspace.view.name, "B");
    assert_eq!(h.app.render_workspace.view.draft.width, 321);
}

#[test]
fn shots_strip_render_submits_the_current_job_request() {
    let mut h = fixture();
    h.app.render_workspace.view.draft.width = 24;
    h.app.render_workspace.view.draft.height = 16;
    h.app.render_workspace.view.draft.background = [31, 57, 83];
    let document = h.app.session.doc.clone();
    h.frames(3);
    h.step(&json!({"click":{"handle":"render:shots:render"}})).unwrap();
    let job = h.app.render_workspace.view.job.unwrap();
    let status = h.app.session.render_job_handle(job).unwrap().status().unwrap();
    assert_eq!((status.request.width, status.request.height), (24, 16));
    assert_eq!(status.request.background, [31, 57, 83]);
    assert_eq!(h.app.session.doc, document);
}

#[test]
fn browser_shot_menus_keep_target_and_draft_through_confirmation() {
    let mut h = fixture();
    h.app.render_workspace.view.name = "A".into();
    setups::save(&mut h.app).unwrap();
    h.app.run("render.settings.set", json!({"name":"B","set":{"width":321,"job":{"method":"raster"}}})).unwrap();
    h.app.render_workspace.view.draft.width = 123;
    h.frames(3);
    h.step(&json!({"click":{"handle":"render:tree:shot:B"},"button":"right"})).unwrap();
    h.step(&json!({"widget":h.app.tr("render.shot_rename")})).unwrap();
    assert!(confirmation_open(&h.app));
    assert_eq!(h.app.render_workspace.view.name, "A");
    h.step(&json!({"widget":h.app.tr("render.keep_editing")})).unwrap();
    assert_eq!(h.app.render_workspace.view.draft.width, 123);
    assert_eq!(h.app.render_workspace.view.inspector.page, Page::Output);
    shot_row_action(&mut h.app, "B".into(), "render.shot.rename");
    h.frames(3);
    h.step(&json!({"widget":h.app.tr("render.discard_continue")})).unwrap();
    assert_eq!(h.app.render_workspace.view.name, "B");
    assert_eq!(h.app.render_workspace.view.inspector.page, Page::Scene);
    assert_eq!(h.app.render_workspace.view.draft.width, 321);
    h.app.render_workspace.view.draft.width = 456;
    shot_row_action(&mut h.app, "A".into(), "render.shot.remove");
    h.frames(3);
    assert!(confirmation_open(&h.app));
    h.step(&json!({"click":{"handle":"dialog:ok"}})).unwrap();
    assert!(
        !h.app.session.doc.render.setups.contains_key("A"),
        "pending={:?}, status={:?}",
        h.app.render_workspace.view.inspector.pending,
        h.app.status
    );
    assert_eq!(h.app.render_workspace.view.name, "B");
    assert_eq!(h.app.render_workspace.view.draft.width, 456, "removing another shot preserves this draft");
}

#[test]
fn appearance_inspector_targets_and_chain_follow_body_face_and_component() {
    use super::selection_appearance::{chains, inspected_look, targets};
    use solidcraft_engine::{Sel, geom::Vec3};
    let mut h = Harness::new();
    h.app.run("component.create", json!({"name":"Part"})).unwrap();
    h.app.run("solid.box", json!({"body_name":"Subject","length":20,"width":15,"height":10})).unwrap();
    h.app.run("material.assign", json!({"bodies":["Subject"],"material":"Steel"})).unwrap();
    h.app.run("appearance.assign", json!({"components":["Part"],"appearance":"ABS - Blue"})).unwrap();
    h.app.run("appearance.assign", json!({"bodies":["Subject"],"appearance":"ABS - Red"})).unwrap();
    h.app.run("appearance.assign", json!({"body":"Subject","faces":[[10,7,10]],"appearance":"Copper"})).unwrap();
    let face = h.app.run("model.faces", json!({"body":"Subject"})).unwrap()["faces"]
        .as_array()
        .unwrap()
        .iter()
        .find(|f| f["point"][2] == 10.0)
        .unwrap()["index"]
        .as_u64()
        .unwrap() as usize;
    h.app.session.selection = vec![Sel::Face { body: "Subject".into(), index: face, point: Vec3::new(10.0, 7.0, 10.0) }];
    let before = h.app.session.doc.to_json();
    for (step, present) in [
        (ui_kit::LookStep::Face, [true, true, true, true]),
        (ui_kit::LookStep::Body, [false, true, true, true]),
        (ui_kit::LookStep::Component, [false, false, true, true]),
    ] {
        let target = targets(&h.app, step);
        let chain = chains(&mut h.app, &target).unwrap();
        assert_eq!(chain[0].each_ref().map(Option::is_some), present);
        assert_eq!(inspected_look(&chain[0], Some(ui_kit::LookStep::Component)).unwrap().name, "ABS - Blue");
        assert_eq!(inspected_look(&chain[0], Some(ui_kit::LookStep::Material)).unwrap().name, "Steel");
        let mut inherited = chain[0].clone();
        inherited[1] = None;
        inherited[2] = None;
        assert_eq!(inspected_look(&inherited, Some(ui_kit::LookStep::Body)).unwrap().name, "Steel");
    }
    assert_eq!(h.app.session.doc.to_json(), before);
}

#[test]
fn appearance_inspector_all_users_replaces_exact_look_once_without_changing_mass() {
    use super::selection_appearance::{assign, chains, targets, users};
    use solidcraft_engine::Sel;
    let mut h = fixture();
    h.app.run("solid.box", json!({"body_name":"Other","length":5,"width":5,"height":5,"corner":[30,0,0]})).unwrap();
    h.app.run("material.assign", json!({"bodies":["Subject","Other"],"material":"Steel"})).unwrap();
    h.app.run("appearance.assign", json!({"bodies":["Subject","Other"],"appearance":"ABS - Red"})).unwrap();
    h.app.session.selection = vec![Sel::Body { name: "Subject".into() }];
    let target = targets(&h.app, ui_kit::LookStep::Body);
    let from = chains(&mut h.app, &target).unwrap()[0][1].clone().unwrap();
    assert_eq!(users(&h.app, &from), 2);
    let mut to = from.clone();
    to.color = [12, 34, 56];
    let mass = h.app.run("component.properties", json!({})).unwrap();
    let undo = h.app.session.undo.len();
    assign(&mut h.app, &target, &from, &to, true).unwrap();
    assert_eq!(h.app.session.undo.len(), undo + 1);
    for name in ["Subject", "Other"] {
        assert_eq!(h.app.session.doc.appearances.bodies.get(name), Some(&to));
    }
    assert_eq!(h.app.run("component.properties", json!({})).unwrap(), mass);
    h.app.run("edit.undo", json!({})).unwrap();
    assert_eq!(users(&h.app, &from), 2);
    h.app.session.selection = vec![Sel::Body { name: "Subject".into() }];
    h.frames(2);
    h.step(&json!({"click":{"handle":"render:stage:look"}})).unwrap();
    assert_eq!(h.app.render_workspace.view.inspector.page, Page::Appearance);
    let undo = h.app.session.undo.len();
    h.step(&json!({"click":{"handle":format!("dialog:{}:{}", h.app.tr("render.inspector_applies_to"), crate::i18n::format("render.inspector_all_users", &[("count", "2")]))}}))
        .unwrap();
    h.step(&json!({"click":{"handle":"render:appearance:preview:replace"}})).unwrap();
    h.step(&json!({"click":{"handle":"dialog: search"}})).unwrap();
    h.step(&json!({"text":"ABS - Blue"})).unwrap();
    let blue = crate::dialogs_appearance::replacement_library(&mut h.app).iter().position(|look| look.name == "ABS - Blue").unwrap();
    h.step(&json!({"click":{"handle":format!("render:appearance:replace:{blue}")}})).unwrap();
    assert_eq!(h.app.session.undo.len(), undo + 1);
    for name in ["Subject", "Other"] {
        assert_eq!(h.app.session.doc.appearances.bodies[name].name, "ABS - Blue");
    }
    let undo = h.app.session.undo.len();
    h.step(&json!({"click":{"handle":format!("dialog:{}:{}",h.app.tr("render.inspector_finish"),h.app.tr("render.inspector_matte"))}})).unwrap();
    assert_eq!(h.app.session.undo.len(), undo + 1, "an all-users field edit is one replacement command");
    for name in ["Subject", "Other"] {
        assert_eq!(h.app.session.doc.appearances.bodies[name].pbr.unwrap().roughness(), 0.85);
    }
    assert_eq!(h.app.run("component.properties", json!({})).unwrap(), mass);
}

#[test]
fn appearance_inspector_finish_presets_mixed_set_all_and_clear_use_existing_assignments() {
    use super::selection_appearance::{assign, chains, clear, finish, mixed, targets};
    use solidcraft_engine::{Sel, doc::appearance::Look};
    let mut h = fixture();
    h.app.run("solid.box", json!({"body_name":"Other","length":5,"width":5,"height":5,"corner":[30,0,0]})).unwrap();
    h.app.run("material.assign", json!({"bodies":["Subject","Other"],"material":"Steel"})).unwrap();
    h.app.run("appearance.assign", json!({"bodies":["Subject"],"appearance":"ABS - Red"})).unwrap();
    h.app.run("appearance.assign", json!({"bodies":["Other"],"appearance":"ABS - Blue"})).unwrap();
    h.app.session.selection = vec![Sel::Body { name: "Subject".into() }, Sel::Body { name: "Other".into() }];
    let target = targets(&h.app, ui_kit::LookStep::Body);
    let looks: Vec<_> = chains(&mut h.app, &target).unwrap().into_iter().map(|c| c[1].clone().unwrap()).collect();
    assert!(mixed(&looks, |l| l.color));
    assert!(!mixed(&looks, |l| l.opacity));
    h.frames(2);
    h.step(&json!({"click":{"handle":"render:stage:look"}})).unwrap();
    assert_eq!(h.app.render_workspace.view.inspector.page, Page::Appearance);
    assert!(h.widget(h.app.tr("render.tree_mixed")).is_some());
    appearance_inspector_capture(&mut h, "mixed");
    let mass = h.app.run("component.properties", json!({})).unwrap();
    let mut to = Look::custom([40, 50, 60], 0.5);
    for (index, metallic, roughness) in [(0, 0.0, 0.85), (1, 0.0, 0.5), (2, 0.0, 0.12), (3, 1.0, 0.32)] {
        finish(&mut to, index).unwrap();
        let pbr = to.pbr.unwrap();
        assert_eq!((pbr.metallic(), pbr.roughness(), to.opacity), (metallic, roughness, 1.0));
    }
    let undo = h.app.session.undo.len();
    assign(&mut h.app, &target, &looks[0], &to, false).unwrap();
    assert_eq!(h.app.session.undo.len(), undo + 1);
    assert_eq!(h.app.session.doc.appearances.bodies.get("Subject"), h.app.session.doc.appearances.bodies.get("Other"));
    clear(&mut h.app, &target, &to).unwrap();
    assert!(h.app.session.doc.appearances.bodies.is_empty());
    assert!(chains(&mut h.app, &target).unwrap().iter().all(|c| c[3].as_ref().is_some_and(|l| l.name == "Steel")));
    assert_eq!(h.app.run("component.properties", json!({})).unwrap(), mass);
}

#[test]
fn appearance_inspector_real_controls_switch_targets_inspect_chain_and_edit_finish() {
    use solidcraft_engine::{Sel, geom::Vec3};
    let mut h = fixture();
    h.app.run("appearance.assign", json!({"bodies":["Subject"],"appearance":"ABS - Red"})).unwrap();
    let face = h.app.run("model.faces", json!({"body":"Subject"})).unwrap()["faces"]
        .as_array()
        .unwrap()
        .iter()
        .find(|f| f["point"][2] == 10.0)
        .unwrap()["index"]
        .as_u64()
        .unwrap() as usize;
    h.app.session.selection = vec![Sel::Face { body: "Subject".into(), index: face, point: Vec3::new(10.0, 7.0, 10.0) }];
    h.frames(2);
    h.step(&json!({"click":{"handle":"render:stage:look"}})).unwrap();
    assert_eq!(h.app.render_workspace.view.inspector.page, Page::Appearance);
    let before = h.app.session.doc.to_json();
    h.step(&json!({"click":{"handle":"render:appearance:chain-menu"}})).unwrap();
    h.step(&json!({"click":{"handle":"render:appearance:chain:body"}})).unwrap();
    assert_eq!(h.app.session.doc.to_json(), before);
    appearance_inspector_capture(&mut h, "face");
    h.frames(3);
    let undo = h.app.session.undo.len();
    h.step(&json!({"click":{"handle":format!("dialog:{}:{}",h.app.tr("render.inspector_finish"),h.app.tr("render.inspector_metal"))}})).unwrap();
    assert_eq!(h.app.session.undo.len(), undo + 1);
    assert_eq!(h.app.session.doc.appearances.bodies["Subject"].pbr.unwrap().metallic(), 1.0);
    let roughness = crate::scenario::rect_of(&format!("dialog:scalar:{}", h.app.tr("appearance.roughness"))).unwrap().center();
    let drag_undo = h.app.session.undo.len();
    h.step(&json!({"drag":[[roughness.x,roughness.y],[roughness.x+20.0,roughness.y]],"steps":12})).unwrap();
    assert_eq!(h.app.session.undo.len(), drag_undo + 1, "a multi-frame field edit is one undo step");
    assert_ne!(h.app.session.doc.appearances.bodies["Subject"].pbr.unwrap().roughness(), 0.32);
    let edited_undo = h.app.session.undo.len();
    h.step(&json!({"click":{"handle":format!("dialog::{}",h.app.tr("render.pick_body"))}})).unwrap();
    assert!(h.app.ui.pick_bodies);
    assert_eq!(h.app.session.undo.len(), edited_undo);
    h.step(&json!({"click":{"handle":format!("dialog::{}",h.app.tr("render.pick_component"))}})).unwrap();
    assert!(h.app.ui.pick_components);
    assert!(h.widget(h.app.tr("render.inspector_pick_target")).is_some(), "root bodies have no component override target");
    h.step(&json!({"click":{"handle":format!("dialog::{}",h.app.tr("render.pick_face"))}})).unwrap();
    assert!(!h.app.ui.pick_components && !h.app.ui.pick_bodies);
    h.step(&json!({"click":{"handle":format!("dialog::{}",h.app.tr("render.pick_body"))}})).unwrap();
    assert_eq!(h.app.session.undo.len(), edited_undo);
    appearance_inspector_capture(&mut h, "body");
    h.app.session.selection.clear();
    h.frames(3);
    assert!(h.widget(h.app.tr("render.base_appearance")).is_some());
    appearance_inspector_capture(&mut h, "explicit-target");
}

#[test]
fn appearance_inspector_mixed_fields_are_inert_until_edited_and_accept_the_first_value() {
    use solidcraft_engine::Sel;
    let mut h = fixture();
    h.app.run("solid.box", json!({"body_name":"Other","length":5,"width":5,"height":5,"corner":[30,0,0]})).unwrap();
    for (name, opacity) in [("Subject", 0.25), ("Other", 0.75)] {
        h.app.run("appearance.assign", json!({"bodies":[name],"color":"#123456","opacity":opacity})).unwrap();
    }
    h.app.session.selection = vec![Sel::Body { name: "Subject".into() }, Sel::Body { name: "Other".into() }];
    h.frames(2);
    h.step(&json!({"click":{"handle":"render:stage:look"}})).unwrap();
    assert_eq!(h.app.render_workspace.view.inspector.page, Page::Appearance);
    let before = h.app.session.doc.to_json();
    let undo = h.app.session.undo.len();
    let field = format!("dialog:scalar:{}", h.app.tr("render.inspector_opacity"));
    h.step(&json!({"click":{"handle":field}})).unwrap();
    h.step(&json!({"key":"Tab"})).unwrap();
    assert_eq!(h.app.session.doc.to_json(), before, "leaving Mixed without editing must be inert");
    assert_eq!(h.app.session.undo.len(), undo);
    h.step(&json!({"click":{"handle":field}})).unwrap();
    h.step(&json!({"text":"NaN"})).unwrap();
    h.step(&json!({"key":"Enter"})).unwrap();
    assert_eq!(h.app.session.doc.to_json(), before, "invalid input must not set mixed targets");
    assert_eq!(h.app.session.undo.len(), undo);
    h.step(&json!({"click":{"handle":field}})).unwrap();
    h.step(&json!({"text":"0.25"})).unwrap();
    assert_eq!(h.app.session.undo.len(), undo, "typing keeps a local draft until commit");
    h.step(&json!({"key":"Enter"})).unwrap();
    assert_eq!(h.app.session.undo.len(), undo + 1);
    for name in ["Subject", "Other"] {
        assert_eq!(h.app.session.doc.appearances.bodies[name].opacity, 0.25);
    }
    h.app.run("edit.undo", json!({})).unwrap();
    assert_eq!(h.app.session.doc.to_json(), before);
}

#[test]
fn appearance_inspector_optical_colour_and_opacity_edits_commit_once_with_shared_bounds() {
    use solidcraft_engine::Sel;
    let mut h = fixture();
    h.app
        .run(
            "appearance.assign",
            json!({"bodies":["Subject"],"color":"#123456",
        "pbr":{"metallic":0.2,"roughness":0.4,"dielectric_ior":1.5}}),
        )
        .unwrap();
    h.app.session.selection = vec![Sel::Body { name: "Subject".into() }];
    h.frames(2);
    h.step(&json!({"click":{"handle":"render:stage:look"}})).unwrap();
    assert_eq!(h.app.render_workspace.view.inspector.page, Page::Appearance);
    let mass = h.app.run("component.properties", json!({})).unwrap();
    for (key, text) in [("appearance.metallic", "2"), ("appearance.roughness", "0"), ("render.inspector_opacity", "0.25")] {
        let undo = h.app.session.undo.len();
        h.step(&json!({"click":{"handle":format!("dialog:scalar:{}",h.app.tr(key))}})).unwrap();
        h.step(&json!({"text":text})).unwrap();
        assert_eq!(h.app.session.undo.len(), undo);
        h.step(&json!({"key":"Enter"})).unwrap();
        assert_eq!(h.app.session.undo.len(), undo + 1, "{key}");
        let look = &h.app.session.doc.appearances.bodies["Subject"];
        match key {
            "appearance.metallic" => assert_eq!(look.pbr.unwrap().metallic(), 1.0),
            "appearance.roughness" => assert_eq!(look.pbr.unwrap().roughness(), 0.045),
            _ => assert_eq!((look.opacity, look.pbr), (0.25, None)),
        }
    }
    let undo = h.app.session.undo.len();
    h.step(&json!({"click":{"handle":format!("dialog:scalar:{}",h.app.tr("render.appearance_editor.ior"))}})).unwrap();
    h.step(&json!({"text":"3.5"})).unwrap();
    h.step(&json!({"key":"Enter"})).unwrap();
    assert_eq!(h.app.session.undo.len(), undo + 1);
    let look = &h.app.session.doc.appearances.bodies["Subject"];
    assert_eq!((look.pbr.unwrap().dielectric_ior(), look.opacity), (3.0, 1.0));
    h.step(&json!({"click":{"handle":format!("dialog:color:{}",h.app.tr("render.inspector_colour"))}})).unwrap();
    let undo = h.app.session.undo.len();
    h.step(&json!({"click":{"handle":"color:channel:R"}})).unwrap();
    h.step(&json!({"text":"200"})).unwrap();
    assert_eq!(h.app.session.undo.len(), undo);
    h.step(&json!({"key":"Enter"})).unwrap();
    assert_eq!(h.app.session.undo.len(), undo + 1);
    assert_eq!(h.app.session.doc.appearances.bodies["Subject"].color[0], 200);
    assert_eq!(h.app.run("component.properties", json!({})).unwrap(), mass);
}

#[test]
fn appearance_inspector_hex_edits_validate_and_commit_once() {
    use solidcraft_engine::Sel;
    let mut h = fixture();
    h.app.run("appearance.assign", json!({"bodies":["Subject"],"appearance":"ABS - Red"})).unwrap();
    h.app.session.selection = vec![Sel::Body { name: "Subject".into() }];
    h.app.run("view.render_stage", json!({"stage":"look"})).unwrap();
    h.frames(3);
    let before = h.app.session.doc.to_json();
    let undo = h.app.session.undo.len();
    let hex = format!("dialog:color_hex:{}", h.app.tr("render.inspector_colour"));
    h.step(&json!({"click":{"handle":hex}})).unwrap();
    h.step(&json!({"key":"A","cmd":true,"ctrl":true})).unwrap();
    h.step(&json!({"text":"#GG0000"})).unwrap();
    h.step(&json!({"key":"Enter"})).unwrap();
    assert_eq!(h.app.session.doc.to_json(), before);
    h.step(&json!({"click":{"handle":hex}})).unwrap();
    h.step(&json!({"key":"A","cmd":true,"ctrl":true})).unwrap();
    h.step(&json!({"text":"#123456"})).unwrap();
    assert_eq!(h.app.session.undo.len(), undo);
    h.step(&json!({"key":"Enter"})).unwrap();
    assert_eq!(h.app.session.doc.appearances.bodies["Subject"].color, [0x12, 0x34, 0x56]);
    assert_eq!(h.app.session.undo.len(), undo + 1);
    h.app.run("solid.box", json!({"body_name":"Other","length":5,"width":5,"height":5,"corner":[30,0,0]})).unwrap();
    h.app.run("appearance.assign", json!({"bodies":["Other"],"color":"#ABCDEF"})).unwrap();
    h.app.session.selection.push(Sel::Body { name: "Other".into() });
    h.frames(3);
    let mixed_before = h.app.session.doc.to_json();
    h.step(&json!({"click":{"handle":hex}})).unwrap();
    h.step(&json!({"key":"Tab"})).unwrap();
    assert_eq!(h.app.session.doc.to_json(), mixed_before);
    h.step(&json!({"click":{"handle":hex}})).unwrap();
    h.step(&json!({"key":"A","cmd":true,"ctrl":true})).unwrap();
    h.step(&json!({"text":"#123456"})).unwrap();
    h.step(&json!({"key":"Enter"})).unwrap();
    assert_eq!(h.app.session.doc.appearances.bodies["Other"].color, [0x12, 0x34, 0x56]);
}

#[test]
fn appearance_inspector_component_and_face_overrides_remove_to_material_from_real_controls() {
    use solidcraft_engine::{Sel, geom::Vec3};
    let mut h = Harness::new();
    h.call("ui.resize", json!({"width":1440,"height":1000}));
    h.app.run("component.create", json!({"name":"Part"})).unwrap();
    h.app.run("solid.box", json!({"body_name":"Subject","length":20,"width":15,"height":10})).unwrap();
    h.app.run("material.assign", json!({"bodies":["Subject"],"material":"Steel"})).unwrap();
    h.app.run("appearance.assign", json!({"components":["Part"],"appearance":"ABS - Blue"})).unwrap();
    h.app.run("appearance.assign", json!({"bodies":["Subject"],"appearance":"ABS - Red"})).unwrap();
    h.app.run("appearance.assign", json!({"body":"Subject","faces":[[10,7,10]],"appearance":"Copper"})).unwrap();
    let face = h.app.run("model.faces", json!({"body":"Subject"})).unwrap()["faces"]
        .as_array()
        .unwrap()
        .iter()
        .find(|f| f["point"][2] == 10.0)
        .unwrap()["index"]
        .as_u64()
        .unwrap() as usize;
    h.app.session.selection = vec![Sel::Face { body: "Subject".into(), index: face, point: Vec3::new(10.0, 7.0, 10.0) }];
    super::super::enter(&mut h.app, true).unwrap();
    h.frames(2);
    h.step(&json!({"click":{"handle":"render:stage:look"}})).unwrap();
    assert_eq!(h.app.render_workspace.view.inspector.page, Page::Appearance);
    let mass = h.app.run("component.properties", json!({})).unwrap();
    for (step, name) in [("face", "Copper"), ("body", "ABS - Red"), ("component", "ABS - Blue"), ("material", "Steel")] {
        let before = h.app.session.doc.to_json();
        h.step(&json!({"click":{"handle":"render:appearance:chain-menu"}})).unwrap();
        h.step(&json!({"click":{"handle":format!("render:appearance:chain:{step}")}})).unwrap();
        assert!(h.widget(name).is_some(), "inspecting {step} must show {name}");
        assert_eq!(h.app.session.doc.to_json(), before);
    }
    h.step(&json!({"click":{"handle":format!("dialog::{}",h.app.tr("render.pick_component"))}})).unwrap();
    appearance_inspector_capture(&mut h, "component");
    h.frames(3);
    for step in ["component", "body", "face"] {
        h.step(&json!({"click":{"handle":format!("dialog::{}",h.app.tr(match step {
            "component" => "render.pick_component", "body" => "render.pick_body", _ => "render.pick_face",
        }))}}))
        .unwrap();
        let undo = h.app.session.undo.len();
        h.step(&json!({"widget":"…"})).unwrap();
        h.step(&json!({"widget":h.app.tr("render.inspector_remove")})).unwrap();
        assert_eq!(h.app.session.undo.len(), undo + 1, "clearing {step}: {:?}", h.app.render_workspace.view.inspector.selection_appearance);
    }
    assert!(h.app.session.doc.appearances.is_empty());
    assert!(h.widget("Steel").is_some());
    assert_eq!(h.app.run("component.properties", json!({})).unwrap(), mass);
}

// Opt-in evidence from the actual application UI, without requiring a window server.
fn appearance_inspector_capture(h: &mut Harness, name: &str) {
    let Some(dir) = std::env::var_os("SOLIDCRAFT_INSPECTOR_EVIDENCE") else { return };
    std::fs::create_dir_all(&dir).unwrap();
    for dark in [false, true] {
        h.app.preferences.theme = if dark { "dark" } else { "light" }.into();
        h.app.ui.dark = dark;
        h.app.preferences.render_inspector_width = 305.0;
        let ctx = egui::Context::default();
        crate::fonts::setup_bundled_fonts(&ctx);
        let mut textures = std::collections::HashMap::new();
        let mut last = None;
        for frame in 0..10 {
            let mut output = ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1440.0, 900.0))),
                    time: Some(frame as f64 / 10.0),
                    focused: true,
                    ..Default::default()
                },
                |ui| {
                    h.app.logic(ui.ctx());
                    h.app.ui(ui);
                },
            );
            ui_kit::update_test_textures(&mut textures, &output.textures_delta);
            output.textures_delta.clear();
            last = Some(output);
        }
        let output = last.unwrap();
        let card = crate::scenario::rect_of("render:inspector").unwrap();
        let crop = ui_kit::raster_ui(&ctx, output.shapes.clone(), output.pixels_per_point, card, &textures);
        crop.save(std::path::Path::new(&dir).join(format!("{name}-{}-305.png", if dark { "dark" } else { "light" }))).unwrap();
        let image = ui_kit::raster_ui(&ctx, output.shapes, output.pixels_per_point, ctx.content_rect(), &textures);
        image.save(std::path::Path::new(&dir).join(format!("{name}-{}.png", if dark { "dark" } else { "light" }))).unwrap();
    }
}

#[test]
fn appearance_inspector_approved_layout_is_305_wide_and_preserves_stage_bounds() {
    use solidcraft_engine::Sel;
    let mut h = fixture();
    h.app.run("solid.box", json!({"body_name":"Boss","length":10,"width":10,"height":10,"corner":[30,0,0]})).unwrap();
    h.app.run("appearance.assign", json!({"bodies":["Boss","Subject"],"appearance":"Aluminum - Anodized Blue"})).unwrap();
    h.app.run("material.assign", json!({"bodies":["Boss","Subject"],"material":"Steel"})).unwrap();
    h.app.run("select.filter", json!({"mode":"body"})).unwrap();
    h.app.session.selection = vec![Sel::Body { name: "Boss".into() }];
    h.app.preferences.render_inspector_width = 305.0;
    h.app.run("view.render_stage", json!({"stage":"look"})).unwrap();
    h.frames(3);
    let before = h.app.session.doc.to_json();
    let undo = h.app.session.undo.len();
    appearance_inspector_capture(&mut h, "approved");
    h.frames(3);
    let inspector = crate::scenario::rect_of("render:inspector").unwrap();
    let browser = crate::scenario::rect_of("browser:card").unwrap();
    let shots = crate::scenario::rect_of("render:shots_strip").unwrap();
    assert!((inspector.width() - 305.0).abs() <= 1.0, "{inspector:?}");
    assert!((inspector.top() - browser.top()).abs() <= 1.0);
    assert!(inspector.bottom() < shots.top());
    assert!(h.widget(&crate::i18n::format("render.inspector_all_users", &[("count", "2")])).is_some());
    assert!(h.widget(h.app.tr("render.inspector_save_library")).is_some());
    assert_eq!(h.app.session.doc.to_json(), before);
    assert_eq!(h.app.session.undo.len(), undo);
    h.step(&json!({"click":{"handle":"render:appearance:close"}})).unwrap();
    assert!(h.app.render_workspace.view.inspector.hidden);
    h.app.run("view.render_stage", json!({"stage":"look"})).unwrap();
    h.frames(3);
    assert!(!h.app.render_workspace.view.inspector.hidden);
    h.step(&json!({"click":{"handle":format!("dialog:{}:{}",h.app.tr("render.inspector_finish"),h.app.tr("render.inspector_matte"))}})).unwrap();
    h.step(&json!({"click":{"handle":format!("dialog:action:{}", h.app.tr("render.inspector_reset"))}})).unwrap();
    assert_eq!(h.app.session.doc.appearances.bodies.get("Boss"), h.app.session.doc.appearances.bodies.get("Subject"));
    assert!(h.app.session.doc.appearances.bodies.contains_key("Subject"));
    assert_eq!(h.app.session.undo.len(), undo + 2);
}

#[test]
fn appearance_editor_e2_sections_are_ordered_collapsed_and_expand_for_each_method() {
    use solidcraft_engine::Sel;
    let mut h = fixture();
    h.call("ui.resize", json!({"width":1440,"height":4000}));
    h.app.preferences.render_inspector_width = 308.0;
    h.app.session.selection = vec![Sel::Body { name: "Subject".into() }];
    h.app.run("view.render_stage", json!({"stage":"look"})).unwrap();
    let titles = [
        "render.inspector_edit",
        "render.appearance_editor.glass",
        "render.appearance_editor.coat",
        "render.appearance_editor.maps",
        "render.appearance_editor.saved",
        "render.appearance_editor.reflections",
    ];
    h.frames(3);
    let before = h.app.session.doc.to_json();
    let mut last = 0.0;
    for title in titles {
        let rect = crate::scenario::rect_of(&format!("dialog:section:{}", h.app.tr(title))).unwrap();
        assert!(rect.top() > last, "{title} follows the preceding design section");
        last = rect.bottom();
    }
    assert!(h.widget(h.app.tr("render.appearance_editor.glass_enable")).is_none(), "optics default to collapsed");
    assert!(h.widget(h.app.tr("render.appearance_editor.gpu_reason")).is_some());
    for title in titles.into_iter().skip(1) {
        h.step(&json!({"click":{"handle":format!("dialog:section:{}",h.app.tr(title))}})).unwrap();
    }
    for method in [ImageMethod::Raster, ImageMethod::GpuRaytrace, ImageMethod::CpuRaytrace] {
        h.app.render_workspace.view.method = method;
        h.frames(3);
        for key in [
            "render.appearance_editor.glass_enable",
            "render.appearance_editor.recipe",
            "render.appearance_editor.boundary",
            "render.appearance_editor.thickness",
            "render.appearance_editor.absorption",
            "render.material_shadow",
            "render.subsurface_enable",
            "render.material_coat_enable",
            "render.appearance_editor.weight",
            "render.material_maps_enable",
            "render.appearance_editor.base_colour",
            "render.appearance_editor.normal",
            "appearance.metallic",
            "appearance.roughness",
            "render.appearance_editor.origin",
            "render.appearance_editor.u_repeat",
            "render.appearance_editor.v_repeat",
            "render.appearance_editor.normal_strength",
            "render.appearance_editor.v_up",
            "render.material_color_replace",
            "render.appearance_editor.save",
            "render.appearance_editor.remove",
            "render.appearance_editor.replace",
            "render.appearance_editor.revalidate",
            "render.cpu_reflection_off",
            "render.cpu_reflection_mirror",
            "render.cpu_reflection_rough",
        ] {
            assert!(h.widget(h.app.tr(key)).is_some(), "expanded E2 is missing {key} for {method:?}");
        }
        let tree = h.output.accesskit_update.as_ref().unwrap();
        let glass = tree.nodes.iter().find(|(_, n)| n.label() == Some(h.app.tr("render.appearance_editor.glass_enable"))).unwrap();
        assert_eq!(glass.1.is_disabled(), method != ImageMethod::GpuRaytrace);
        assert_eq!(h.app.session.doc.to_json(), before);
        assert!(h.app.render_workspace.view.materials.bindings.is_empty(), "opening never installs temporary bindings");
    }
    for selection in
        [Sel::Face { body: "Subject".into(), index: 0, point: solidcraft_engine::geom::Vec3::ZERO }, Sel::Body { name: "Subject".into() }]
    {
        h.app.session.selection = vec![selection];
        if matches!(h.app.session.selection.first(), Some(Sel::Body { .. })) {
            h.app.ui.pick_components = true;
        }
        h.frames(3);
        assert!(h.widget(h.app.tr("render.appearance_editor.body_reason")).is_some());
        for title in titles.into_iter().skip(1) {
            assert!(crate::scenario::rect_of(&format!("dialog:section:{}", h.app.tr(title))).is_some());
        }
    }
}

#[test]
fn appearance_editor_reset_restores_each_picked_saved_look() {
    use solidcraft_engine::Sel;
    let mut h = fixture();
    h.app.run("appearance.assign", json!({"bodies":["Subject"],"appearance":"ABS - Red"})).unwrap();
    h.app.session.selection = vec![Sel::Body { name: "Subject".into() }];
    h.app.run("view.render_stage", json!({"stage":"look"})).unwrap();
    h.frames(3);
    let saved = h.app.session.doc.appearances.bodies["Subject"].clone();
    let target = selection_appearance::targets(&h.app, ui_kit::LookStep::Body);
    let changed = solidcraft_engine::doc::appearance::Look::custom([10, 20, 30], 0.3);
    selection_appearance::assign(&mut h.app, &target, &saved, &changed, false).unwrap();
    h.frames(3);
    selection_appearance::reset(&mut h.app);
    assert_eq!(h.app.session.doc.appearances.bodies["Subject"], saved);
}

#[test]
fn appearance_editor_saved_optics_removal_confirms_and_preserves_other_drafts() {
    use solidcraft_engine::Sel;
    let mut h = fixture();
    h.call("ui.resize", json!({"width":1440,"height":2200}));
    h.app.render_workspace.view.method = ImageMethod::GpuRaytrace;
    let set = setups::snapshot(&h.app).unwrap();
    let name = h.app.render_workspace.view.name.clone();
    h.app
        .run(
            "render.optics.bind",
            json!({"name":name,"set":set,"body_index":0,"body_name":"Subject",
        "optics":{"ior":1.5,"absorption_per_mm":[0.0,0.0,0.0],"boundary":"closed_mesh","thickness_mm":0.0,"shadow_transmission":false},
        "expected_revision":h.app.session.revision}),
        )
        .unwrap();
    setups::load(&mut h.app);
    h.app.session.selection = vec![Sel::Body { name: "Subject".into() }];
    h.app.run("view.render_stage", json!({"stage":"look"})).unwrap();
    h.frames(3);
    h.step(&json!({"click":{"handle":format!("dialog:section:{}",h.app.tr("render.appearance_editor.saved"))}})).unwrap();
    let before = h.app.session.doc.to_json();
    h.step(&json!({"click":{"handle":"render:optics-remove"}})).unwrap();
    assert!(materials::confirmation_open(&h.app));
    assert!(h.widget(h.app.tr("render.appearance_editor.remove_title")).is_some());
    assert_eq!(h.app.session.doc.to_json(), before);
    h.step(&json!({"click":{"handle":"dialog:cancel"}})).unwrap();
    assert!(!materials::confirmation_open(&h.app));
    assert_eq!(h.app.session.doc.to_json(), before);
    h.step(&json!({"click":{"handle":"render:optics-remove"}})).unwrap();
    h.app.run("appearance.assign", json!({"bodies":["Subject"],"appearance":"ABS - Red"})).unwrap();
    h.frames(3);
    assert!(!materials::confirmation_open(&h.app), "a changed document expires the pending confirmation");
    assert_eq!(h.app.session.doc.render.setups[&name].optics.len(), 1);
    h.app.render_workspace.view.draft.width = 777;
    h.frames(3);
    h.step(&json!({"click":{"handle":"render:optics-remove"}})).unwrap();
    h.step(&json!({"click":{"handle":"dialog:ok"}})).unwrap();
    assert!(h.app.session.doc.render.setups[&name].optics.is_empty());
    assert!(h.app.render_workspace.view.draft.optics.is_empty());
    assert_eq!(h.app.render_workspace.view.draft.width, 777);
    assert_eq!(h.app.session.doc.appearances.bodies["Subject"].name, "ABS - Red");
}

#[test]
fn appearance_editor_save_optics_keeps_other_temporary_bindings() {
    use solidcraft_engine::Sel;
    let mut h = fixture();
    h.call("ui.resize", json!({"width":1440,"height":2600}));
    h.app.run("solid.box", json!({"body_name":"Other","length":5,"width":5,"height":5,"corner":[30,0,0]})).unwrap();
    h.app.render_workspace.view.method = ImageMethod::GpuRaytrace;
    let other = materials::read_targets(&mut h.app).unwrap().into_iter().find(|t| t.name == "Other").unwrap();
    let mut binding = materials::Binding::new(other, &h.app);
    binding.coat_enabled = true;
    h.app.render_workspace.view.materials.bindings.push(binding);
    h.app.render_workspace.view.materials.enabled = true;
    h.app.session.selection = vec![Sel::Body { name: "Subject".into() }];
    h.app.run("view.render_stage", json!({"stage":"look"})).unwrap();
    h.frames(3);
    h.step(&json!({"click":{"handle":format!("dialog:section:{}",h.app.tr("render.appearance_editor.glass"))}})).unwrap();
    h.step(&json!({"widget":h.app.tr("render.material_custom")})).unwrap();
    h.step(&json!({"click":{"handle":"render:glass:starter.glass.amber-tint"}})).unwrap();
    h.step(&json!({"click":{"handle":format!("dialog:section:{}",h.app.tr("render.appearance_editor.saved"))}})).unwrap();
    let geometry = h.app.run("component.properties", json!({})).unwrap();
    let undo = h.app.session.undo.len();
    h.step(&json!({"click":{"handle":"render:optics-save"}})).unwrap();
    let view = &h.app.render_workspace.view;
    assert_eq!(h.app.session.doc.render.setups[&view.name].optics.len(), 1);
    assert_eq!(view.materials.bindings.len(), 1);
    assert_eq!(view.materials.bindings[0].target.name, "Other");
    assert!(view.materials.bindings[0].coat_enabled);
    assert_eq!(h.app.session.undo.len(), undo + 1);
    assert_eq!(h.app.run("component.properties", json!({})).unwrap(), geometry);
}
