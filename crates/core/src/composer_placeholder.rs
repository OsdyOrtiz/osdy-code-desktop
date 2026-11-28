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
];

/// The line for `seed`, any value (a random one per tab).
pub fn pick(seed: u64) -> &'static str {
    LINES[(seed % LINES.len() as u64) as usize]
}

#[cfg(test)]
mod tests {
    use super::{LINES, pick};
    use crate::i18n::{Language, tr};

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
    fn the_spanish_lines_are_the_ones_asked_for() {
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
            ]
        );
    }
}
