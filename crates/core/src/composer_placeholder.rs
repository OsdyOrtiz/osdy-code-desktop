//! The composer's placeholder: one playful line, picked at random per tab.
//! The English lines are the lookup keys; Spanish copies live in
//! `i18n::es::agent_session`.

/// Every line the composer can show while empty.
pub const LINES: &[&str] = &[
    "Alright then. Let's cook…",
    "Cooking at terminal heat…",
    "Flipping the tortilla…",
    "Getting it just right…",
    "The coffee kicked in…",
    "Moving imaginary cables…",
    "Sorting out the mess…",
    "Looking for where the mess started…",
    "Negotiating with the compiler…",
    "Asking the code what it meant…",
    "Following the trail of a comma…",
    "Checking whether “it's a tiny change” was true…",
    "Yesterday's TODO says hi…",
    "Reading the comment that says “do not touch”…",
    "Untangling spaghetti, one noodle at a time…",
    "Calling on the patron saint of semicolons…",
    "This smells like an environment variable…",
    "Heads up, something fishy is going on here…",
    "The raccoon found a TODO from 2019…",
    "The raccoon snuck into the repo…",
    "The raccoon asked for another cup of coffee…",
    "Seasoning the code with a pinch of Git…",
    "Adding a little refried bean spread to the refactor…",
    "This started as a simple baleada…",
    "Giving that bug a good whack…",
    "Figuring out what “final fix FINAL” was supposed to mean…",
    "Opening the file nobody dares to open…",
    "Asking Git who did it. It was me…",
    "Reading the README before someone tells me to…",
    "Translating “it works on my machine”…",
    "Setting up a meeting between the code and the docs…",
    "The three-line function just lawyered up…",
    "Looking for the zero that passed itself off as an O…",
    "Taking the “just in case” bits out of the code…",
    "The legacy code just called me “kid”…",
    "Well then, there’s more history here than the ticket let on…",
];

/// The line for `seed`, any value (a random one per tab).
pub fn pick(seed: u64) -> &'static str {
    LINES[(seed % LINES.len() as u64) as usize]
}

/// Time between dot frames.
pub const DOT_INTERVAL: std::time::Duration = std::time::Duration::from_millis(400);

/// Replace only a trailing ellipsis with the frame's zero to three dots.
pub fn ellipsis_frame(line: &str, elapsed: std::time::Duration) -> std::borrow::Cow<'_, str> {
    let Some(stem) = line.strip_suffix('…') else {
        return line.into();
    };
    const DOTS: [&str; 4] = ["", ".", "..", "..."];
    let frame = (elapsed.as_millis() / DOT_INTERVAL.as_millis() % DOTS.len() as u128) as usize;
    if frame == 0 {
        stem.into()
    } else {
        format!("{stem}{}", DOTS[frame]).into()
    }
}

#[cfg(test)]
mod tests {
    use super::{LINES, ellipsis_frame, pick};
    use crate::i18n::{Language, tr};
    use std::time::Duration;

    #[test]
    fn every_seed_lands_on_a_line_and_each_line_is_reachable() {
        for seed in 0..LINES.len() as u64 * 2 {
            assert!(LINES.contains(&pick(seed)));
        }
        let mut seen: Vec<_> = (0..LINES.len() as u64).map(pick).collect();
        seen.sort();
        seen.dedup();
        assert_eq!(seen.len(), LINES.len());
        assert!(LINES.contains(&pick(u64::MAX)));
    }

    #[test]
    fn trailing_ellipsis_cycles_every_400_ms() {
        for (millis, expected) in [
            (0, "A… B"),
            (399, "A… B"),
            (400, "A… B."),
            (799, "A… B."),
            (800, "A… B.."),
            (1199, "A… B.."),
            (1200, "A… B..."),
            (1599, "A… B..."),
            (1600, "A… B"),
        ] {
            assert_eq!(
                ellipsis_frame("A… B…", Duration::from_millis(millis)),
                expected
            );
        }
        assert_eq!(
            ellipsis_frame("No trailing ellipsis", Duration::from_millis(800)),
            "No trailing ellipsis"
        );
    }

    #[test]
    fn the_spanish_lines_are_the_ones_asked_for() {
        assert_eq!(LINES.len(), 36);
        assert!(LINES.iter().all(|en| tr(Language::Spanish, en) != *en));
        let spanish: Vec<&str> = LINES.iter().map(|en| tr(Language::Spanish, en)).collect();
        assert_eq!(
            spanish,
            [
                "Dale, pues. A cocinar…",
                "Cocinando a fuego de terminal…",
                "Dándole vuelta a la tortilla…",
                "Lo estoy dejando al punto…",
                "El café ya hizo efecto…",
                "Moviendo cables imaginarios…",
                "Acomodando el relajo…",
                "Buscando dónde se armó el relajo…",
                "Negociando con el compilador…",
                "Preguntándole al código qué quiso decir…",
                "Siguiendo el rastro de una coma…",
                "Viendo si «es un cambiecito» era cierto…",
                "El TODO de ayer mandó saludos…",
                "Leyendo el comentario que dice «no tocar»…",
                "Desenredando espagueti, un fideo a la vez…",
                "Invocando al santo del punto y coma…",
                "Esto huele a variable de entorno…",
                "Pilas, que aquí hay gato encerrado…",
                "El mapache encontró un TODO del 2019…",
                "El mapache se metió al repo…",
                "El mapache pidió otra taza de café…",
                "Sazonando el código con una pizca de Git…",
                "Poniéndole frijolitos al refactor…",
                "Esto empezó como una baleada sencilla…",
                "Dándole maceta al bug…",
                "Viendo qué quiso decir «arreglo final FINAL»…",
                "Abriendo el archivo que nadie se atreve a abrir…",
                "Preguntándole a Git quién fue. Fui yo…",
                "Leyendo el README antes de que me lo reclamen…",
                "Traduciendo «en mi máquina sí funciona»…",
                "Organizando una reunión entre el código y la documentación…",
                "La función de tres líneas acaba de pedir abogado…",
                "Buscando el cero que se hizo pasar por una O…",
                "Quitándole los «por si acaso» al código…",
                "El código heredado me acaba de decir «joven»…",
                "Vaya pues, aquí hay más historia de la que decía el ticket…",
            ]
        );
    }
}
