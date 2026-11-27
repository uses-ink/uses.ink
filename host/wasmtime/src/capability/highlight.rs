use engine::Record as _;
use model::Diagnostic;
use model::Problem;
use model::highlight::Highlight as _;

use super::State;
use crate::bind::plugin::ink::host::highlight;

impl highlight::Host for State {
    fn highlight(&mut self, language: String, code: String) -> Result<Vec<highlight::Run>, highlight::Unhighlighted> {
        let runs =
            self.grammars.highlight(&language, &code).map_err(|refused| highlight::Unhighlighted::from(&refused))?;
        runs.into_iter().map(highlight::Run::try_from).collect::<Result<_, _>>().map_err(|unmirrored| {
            let problem = Problem::PluginValueUnmirrored { reason: unmirrored.to_string() };
            self.said.diagnose(Diagnostic::new(problem));
            highlight::Unhighlighted::Failed
        })
    }
}
