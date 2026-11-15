//! Film simulations: one-click looks grouped by kind (film stocks,
//! rangefinder camera rendering, nature, movie looks). Names describe the
//! style; they are not product or movie titles.

use crate::finish::Look;

pub struct FilmSim {
    pub group: &'static str,
    pub name: &'static str,
    pub note: &'static str,
    pub look: Look,
}

/// Groups in the order they are shown.
pub const GROUPS: [&str; 5] = ["Film stock", "Camera film modes", "Rangefinder", "Nature", "Movie looks"];

pub fn all() -> Vec<FilmSim> {
    let n = Look::default();
    let sim = |group, name, note, look| FilmSim { group, name, note, look };
    vec![
        // ----- Film stock -----
        sim(
            "Film stock",
            "Portrait",
            "Warm, soft contrast, gentle skin tones",
            Look {
                warmth: 0.2,
                contrast: -0.05,
                s_curve: 0.15,
                shadows: 0.15,
                highlights: -0.2,
                saturation: -0.05,
                tint_shadows: [0.0, 0.01, 0.025],
                tint_highlights: [0.03, 0.015, 0.0],
                grain: 0.15,
                ..n
            },
        ),
        sim(
            "Film stock",
            "Vivid Slide",
            "Rich color and deep contrast",
            Look { saturation: 0.45, s_curve: 0.45, shadows: -0.15, warmth: 0.05, vignette: 0.15, grain: 0.05, ..n },
        ),
        sim(
            "Film stock",
            "Muted Chrome",
            "Low saturation, firm contrast, cool shadows",
            Look {
                saturation: -0.25,
                s_curve: 0.25,
                highlights: -0.2,
                tint_shadows: [0.0, 0.01, 0.035],
                tint_highlights: [0.02, 0.015, 0.0],
                grain: 0.1,
                ..n
            },
        ),
        sim(
            "Film stock",
            "Gold",
            "Warm, golden everyday film",
            Look {
                warmth: 0.45,
                saturation: 0.15,
                s_curve: 0.2,
                fade: 0.1,
                tint_highlights: [0.04, 0.02, -0.01],
                grain: 0.25,
                ..n
            },
        ),
        sim(
            "Film stock",
            "Faded Negative",
            "Lifted blacks, soft and cool",
            Look {
                fade: 0.45,
                contrast: -0.15,
                s_curve: -0.1,
                warmth: 0.1,
                tint_shadows: [0.0, 0.015, 0.03],
                grain: 0.3,
                ..n
            },
        ),
        sim(
            "Film stock",
            "Instant",
            "Washed-out instant photo",
            Look {
                fade: 0.35,
                warmth: 0.25,
                saturation: -0.15,
                highlights: -0.25,
                tint_shadows: [0.02, 0.0, 0.03],
                vignette: 0.25,
                grain: 0.2,
                ..n
            },
        ),
        sim(
            "Film stock",
            "Cross Process",
            "Yellow-green highlights, blue shadows",
            Look {
                saturation: 0.25,
                s_curve: 0.35,
                tint_shadows: [-0.02, 0.0, 0.06],
                tint_highlights: [0.03, 0.05, -0.04],
                grain: 0.15,
                ..n
            },
        ),
        sim(
            "Film stock",
            "Bleach Bypass",
            "Silvery, low color, strong contrast",
            Look { saturation: -0.55, s_curve: 0.5, highlights: 0.1, grain: 0.2, ..n },
        ),
        sim(
            "Film stock",
            "Classic B&W",
            "Contrasty black and white with grain",
            Look { saturation: -1.0, s_curve: 0.35, vignette: 0.2, grain: 0.35, ..n },
        ),
        sim(
            "Film stock",
            "Soft B&W",
            "Gentle, faded black and white",
            Look { saturation: -1.0, fade: 0.3, s_curve: 0.1, grain: 0.2, ..n },
        ),
        sim(
            "Film stock",
            "Sepia",
            "Warm brown-toned black and white",
            Look {
                saturation: -1.0,
                s_curve: 0.15,
                fade: 0.15,
                tint_shadows: [0.03, 0.01, -0.01],
                tint_highlights: [0.06, 0.03, -0.02],
                grain: 0.2,
                ..n
            },
        ),
        // ----- Camera film modes: modeled on the in-camera film simulations of
        // a well-known mirrorless camera maker (descriptive names only) -----
        sim(
            "Camera film modes",
            "Standard Slide",
            "Balanced everyday color with a little punch",
            Look { s_curve: 0.2, saturation: 0.1, ..n },
        ),
        sim(
            "Camera film modes",
            "Soft Slide",
            "Soft contrast, smooth skin, bright color",
            Look {
                s_curve: 0.1,
                shadows: 0.1,
                highlights: -0.15,
                saturation: 0.15,
                warmth: 0.05,
                tint_highlights: [0.01, 0.005, 0.0],
                ..n
            },
        ),
        sim(
            "Camera film modes",
            "Studio Negative",
            "Low contrast, neutral color for portraits",
            Look { contrast: -0.1, s_curve: 0.05, saturation: -0.1, highlights: -0.1, shadows: 0.05, ..n },
        ),
        sim(
            "Camera film modes",
            "Cinema Flat",
            "Very soft, low saturation, teal shadows (video-like)",
            Look {
                contrast: -0.15,
                s_curve: -0.05,
                saturation: -0.3,
                shadows: 0.15,
                highlights: -0.25,
                tint_shadows: [-0.01, 0.01, 0.025],
                tint_highlights: [0.01, 0.01, 0.0],
                ..n
            },
        ),
        sim(
            "Camera film modes",
            "Street Negative",
            "Hard contrast, muted color, cyan shadows and warm highlights",
            Look {
                s_curve: 0.4,
                saturation: -0.15,
                highlights: -0.1,
                fade: 0.05,
                tint_shadows: [-0.02, 0.015, 0.025],
                tint_highlights: [0.035, 0.01, -0.01],
                grain: 0.15,
                ..n
            },
        ),
        sim(
            "Camera film modes",
            "Amber Negative",
            "Amber highlights and rich shadows, nostalgic",
            Look {
                warmth: 0.3,
                s_curve: 0.25,
                saturation: 0.05,
                shadows: -0.05,
                tint_shadows: [0.0, 0.0, 0.01],
                tint_highlights: [0.045, 0.025, -0.02],
                ..n
            },
        ),
        sim(
            "Camera film modes",
            "Fine Mono",
            "Smooth black and white with fine grain",
            Look { saturation: -1.0, s_curve: 0.3, shadows: -0.05, highlights: 0.05, grain: 0.12, grain_size: 1.0, ..n },
        ),
        sim(
            "Camera film modes",
            "True Color",
            "Accurate, slightly crisp color",
            Look { s_curve: 0.15, saturation: 0.05, highlights: -0.05, ..n },
        ),
        // ----- Rangefinder: rich blacks, restrained color, strong vignette -----
        sim(
            "Rangefinder",
            "Rangefinder Classic",
            "Deep blacks, warm restrained color, soft vignette",
            Look {
                s_curve: 0.35,
                shadows: -0.2,
                highlights: -0.1,
                saturation: -0.05,
                warmth: 0.08,
                vignette: 0.25,
                tint_highlights: [0.015, 0.008, 0.0],
                grain: 0.05,
                ..n
            },
        ),
        sim(
            "Rangefinder",
            "Rangefinder Vivid",
            "Punchy color with dense shadows",
            Look { s_curve: 0.3, contrast: 0.1, saturation: 0.25, warmth: 0.05, shadows: -0.1, vignette: 0.2, ..n },
        ),
        sim(
            "Rangefinder",
            "Rangefinder Mono",
            "Crisp black and white, deep blacks, fine grain",
            Look { saturation: -1.0, s_curve: 0.45, shadows: -0.2, highlights: 0.05, vignette: 0.3, grain: 0.15, ..n },
        ),
        sim(
            "Rangefinder",
            "Rangefinder Selenium",
            "Black and white with a cool, slightly purple tone",
            Look {
                saturation: -1.0,
                s_curve: 0.35,
                shadows: -0.1,
                tint_shadows: [0.01, 0.0, 0.03],
                tint_highlights: [0.01, 0.0, 0.015],
                vignette: 0.2,
                grain: 0.1,
                ..n
            },
        ),
        // ----- Nature -----
        sim(
            "Nature",
            "Wet Forest",
            "Greens pushed toward teal by a green S-curve, damp and cool",
            Look {
                green_shift: 0.8,
                green_curve: 0.5,
                s_curve: 0.2,
                saturation: 0.1,
                highlights: -0.2,
                shadows: -0.1,
                warmth: -0.1,
                exposure: -0.1,
                tint_shadows: [-0.02, 0.01, 0.02],
                grain: 0.1,
                ..n
            },
        ),
        sim(
            "Nature",
            "Deep Forest",
            "Darker, moodier teal greens with crushed shadows",
            Look {
                green_shift: 0.85,
                green_curve: 0.6,
                s_curve: 0.35,
                shadows: -0.25,
                highlights: -0.15,
                saturation: -0.05,
                warmth: -0.15,
                exposure: -0.2,
                vignette: 0.3,
                tint_shadows: [-0.02, 0.015, 0.03],
                grain: 0.15,
                ..n
            },
        ),
        // ----- Movie looks (named after genres, not titles) -----
        sim(
            "Movie looks",
            "Cinema Teal",
            "Teal shadows, warm highlights (blockbuster)",
            Look {
                s_curve: 0.3,
                saturation: 0.1,
                tint_shadows: [-0.03, 0.02, 0.05],
                tint_highlights: [0.05, 0.02, -0.03],
                grain: 0.1,
                ..n
            },
        ),
        sim(
            "Movie looks",
            "Desert Epic",
            "Hot orange highlights, dusty contrast",
            Look {
                warmth: 0.5,
                s_curve: 0.35,
                saturation: -0.1,
                fade: 0.05,
                vignette: 0.2,
                tint_shadows: [0.02, 0.0, -0.02],
                tint_highlights: [0.05, 0.03, -0.03],
                grain: 0.1,
                ..n
            },
        ),
        sim(
            "Movie looks",
            "Neon Noir",
            "Cyan and magenta light, crushed blacks (sci-fi night)",
            Look {
                s_curve: 0.45,
                shadows: -0.25,
                saturation: 0.3,
                warmth: -0.15,
                vignette: 0.3,
                tint_shadows: [0.0, 0.02, 0.06],
                tint_highlights: [0.05, 0.0, 0.04],
                grain: 0.1,
                ..n
            },
        ),
        sim(
            "Movie looks",
            "Digital Green",
            "Cool green cast, flat color (techno thriller)",
            Look {
                saturation: -0.2,
                s_curve: 0.3,
                warmth: -0.1,
                tint_shadows: [-0.01, 0.04, 0.0],
                tint_highlights: [0.0, 0.03, -0.01],
                grain: 0.05,
                ..n
            },
        ),
        sim(
            "Movie looks",
            "Pastel Storybook",
            "Soft pastel colors, lifted and warm (whimsical comedy)",
            Look {
                fade: 0.3,
                saturation: 0.15,
                warmth: 0.15,
                s_curve: -0.05,
                highlights: -0.1,
                tint_highlights: [0.03, 0.01, 0.02],
                ..n
            },
        ),
        sim(
            "Movie looks",
            "War Drama",
            "Desaturated, harsh contrast, heavy grain",
            Look {
                saturation: -0.6,
                s_curve: 0.5,
                warmth: -0.05,
                highlights: 0.1,
                tint_shadows: [0.0, 0.01, 0.015],
                grain: 0.3,
                ..n
            },
        ),
        sim(
            "Movie looks",
            "Noir",
            "High-contrast black and white, dark edges (detective film)",
            Look { saturation: -1.0, s_curve: 0.6, shadows: -0.3, vignette: 0.45, grain: 0.25, ..n },
        ),
    ]
}

/// A film look blended with no change: 0 = untouched, 1 = full style.
pub fn at_strength(look: &Look, strength: f32) -> Look {
    look.scaled(strength)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strength_blends_from_nothing_to_the_full_style() {
        let sims = all();
        assert!(sims.len() >= 30);
        assert!(GROUPS.iter().all(|g| sims.iter().any(|s| s.group == *g)));
        assert!(sims.iter().all(|s| GROUPS.contains(&s.group)));
        let film = &sims[0].look;
        assert_eq!(at_strength(film, 0.0), Look::default());
        assert_eq!(at_strength(film, 1.0), *film);
        let half = at_strength(film, 0.5);
        assert!((half.warmth - film.warmth / 2.0).abs() < 1e-6);
    }

    #[test]
    fn sepia_tints_a_gray_picture_warm() {
        let mut img = image::RgbaImage::from_pixel(4, 4, image::Rgba([128, 128, 128, 255]));
        let sepia = all().into_iter().find(|f| f.name == "Sepia").unwrap().look;
        crate::finish::apply_tone(&mut img, &sepia);
        let p = img.get_pixel(1, 1);
        assert!(p[0] > p[2], "red above blue: {p:?}");
    }

    #[test]
    fn wet_forest_makes_leaves_teal() {
        let mut img = image::RgbaImage::from_pixel(4, 4, image::Rgba([70, 150, 60, 255]));
        let wet = all().into_iter().find(|f| f.name == "Wet Forest").unwrap().look;
        crate::finish::apply_tone(&mut img, &wet);
        let p = img.get_pixel(1, 1);
        // Teal: blue close to green, red well below both.
        assert!((p[1] as i32 - p[2] as i32).abs() < 45 && p[0] < p[2], "{p:?}");
    }
}
