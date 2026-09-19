"""One-time, guarded source migration for the initial native observer.

The development runner applies this before rustfmt. No game/runtime dependency.
Only the two audited source blobs are accepted; rerunning on migrated files is a no-op.
"""
from pathlib import Path
import subprocess

ROOT = Path(__file__).resolve().parents[2]


def migrate(relative, expected_blob, marker, replacements):
    path = ROOT / relative
    source = path.read_text()
    if marker in source:
        print(f"Already migrated: {relative}")
        return
    actual = subprocess.check_output(
        ["git", "hash-object", str(path)], text=True
    ).strip()
    if actual != expected_blob:
        raise SystemExit(f"Refusing to migrate modified source: {relative}: {actual}")
    for old, new, count in replacements:
        if source.count(old) != count:
            raise SystemExit(f"Unexpected match count in {relative}: {old!r}")
        source = source.replace(old, new)
    path.write_text(source)
    print(f"Migrated: {relative}")


migrate(
    "apps/osb-client/src/ui.rs",
    "90d9c43057ed4ea37b93a3ff082753ab5fb4f883",
    "let mut viewport_ui = egui::Ui::new(",
    [
        (", SoldierId,", ",", 1),
        ("(*ctx.style()).clone()", "(*ctx.style_of(egui::Theme::Dark)).clone()", 1),
        ("ctx.set_style(style);", "ctx.set_theme(egui::Theme::Dark);\n        ctx.set_style_of(egui::Theme::Dark, style);", 1),
        ("ctx.wants_keyboard_input()", "ctx.egui_wants_keyboard_input()", 1),
        (
            '    egui::TopBottomPanel::top("header")',
            '    // bevy_egui owns the pass; do not call Context::run_ui here.\n'
            '    let mut viewport_ui = egui::Ui::new(\n'
            '        ctx.clone(),\n'
            '        "osb-viewport".into(),\n'
            '        egui::UiBuilder::new()\n'
            '            .layer_id(egui::LayerId::background())\n'
            '            .max_rect(ctx.viewport_rect()),\n'
            '    );\n'
            '    egui::Panel::top("header")',
            1,
        ),
        ("egui::TopBottomPanel::bottom", "egui::Panel::bottom", 1),
        ("egui::SidePanel::", "egui::Panel::", 2),
        (".min_height(", ".min_size(", 2),
        (".min_width(", ".min_size(", 2),
        (".default_height(", ".default_size(", 1),
        (".default_width(", ".default_size(", 2),
        (".show(ctx, |ui| {", ".show(&mut viewport_ui, |ui| {", 5),
    ],
)
migrate(
    "apps/osb-client/src/main.rs",
    "ef7bfcbd141b293e55bacd8fde9ccb91f8dede7e",
    ".insert_non_send(state)",
    [
        ("use bevy_egui::{EguiPlugin, EguiPrimaryContextPass};", "use bevy_egui::{EguiPlugin, EguiPrimaryContextPass, EguiStartupSet};", 1),
        (".insert_non_send_resource(state)", ".insert_non_send(state)", 1),
        (".add_systems(Startup, setup)", ".add_systems(PreStartup, setup.before(EguiStartupSet::InitContexts))", 1),
    ],
)
