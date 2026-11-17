use std::sync::Arc;

use eframe::egui::{self, FontData, FontDefinitions, FontFamily, FontId};

/// Font weights the views use. Each one is a named egui font family.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum W {
    Regular,
    Medium,
    SemiBold,
    Bold,
    Mono,
    MonoMedium,
}

impl W {
    fn family_name(self) -> &'static str {
        match self {
            W::Regular => "ui",
            W::Medium => "ui-medium",
            W::SemiBold => "ui-semibold",
            W::Bold => "ui-bold",
            W::Mono => "mono",
            W::MonoMedium => "mono-medium",
        }
    }
}

pub fn font(w: W, size: f32) -> FontId {
    FontId::new(size, FontFamily::Name(w.family_name().into()))
}

const FILES: [(&str, &[u8]); 6] = [
    ("montserrat-regular", include_bytes!("../../../../assets/fonts/Montserrat-Regular.ttf")),
    ("montserrat-medium", include_bytes!("../../../../assets/fonts/Montserrat-Medium.ttf")),
    ("montserrat-semibold", include_bytes!("../../../../assets/fonts/Montserrat-SemiBold.ttf")),
    ("montserrat-bold", include_bytes!("../../../../assets/fonts/Montserrat-Bold.ttf")),
    ("jetbrains-regular", include_bytes!("../../../../assets/fonts/JetBrainsMono-Regular.ttf")),
    ("jetbrains-medium", include_bytes!("../../../../assets/fonts/JetBrainsMono-Medium.ttf")),
];

pub fn install_fonts(ctx: &egui::Context) {
    let mut defs = FontDefinitions::default();
    for (name, bytes) in FILES {
        defs.font_data.insert(name.into(), Arc::new(FontData::from_static(bytes)));
    }
    // egui's own fonts stay as fallbacks for glyphs that Montserrat lacks.
    let fallback = defs.families.get(&FontFamily::Proportional).cloned().unwrap_or_default();
    let family = |first: &str| {
        let mut v = vec![first.to_string()];
        v.extend(fallback.iter().cloned());
        v
    };
    defs.families.insert(FontFamily::Proportional, family("montserrat-regular"));
    defs.families.insert(FontFamily::Monospace, family("jetbrains-regular"));
    for (w, file) in [
        (W::Regular, "montserrat-regular"),
        (W::Medium, "montserrat-medium"),
        (W::SemiBold, "montserrat-semibold"),
        (W::Bold, "montserrat-bold"),
        (W::Mono, "jetbrains-regular"),
        (W::MonoMedium, "jetbrains-medium"),
    ] {
        defs.families.insert(FontFamily::Name(w.family_name().into()), family(file));
    }
    ctx.set_fonts(defs);
}
