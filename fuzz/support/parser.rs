//! Part 21 streaming parser under the shared fuzz budgets: events until the first error.
#[path = "../limits.rs"]
mod budgets;
pub fn exercise(data: &[u8]) {
    for event in tessstep_part21::Parser::new(data, budgets::limits()).unwrap() {
        if event.is_err() {
            break;
        }
    }
}
