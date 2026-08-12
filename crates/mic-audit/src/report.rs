#![forbid(unsafe_code)]
//! Human-readable reports that open with assumptions and abstentions.

use crate::{
    CertificateGates, CertificateStatus, EvidenceLedger, ExecutionMode, Finding, Severity,
};
use serde::Serialize;
use std::fmt::Write as _;

/// Markdown + JSON report envelope.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct NarrativeReport {
    /// Schema version.
    schema_version: String,
    /// Experiment identifier.
    experiment_id: String,
    /// Conservative certificate status.
    status: CertificateStatus,
    /// Complete typed inputs from which `status` was derived.
    gates: CertificateGates,
    /// Execution policy.
    mode: ExecutionMode,
    /// Markdown that leads with assumptions and abstentions.
    markdown: String,
}

impl NarrativeReport {
    /// Returns the report schema version.
    #[must_use]
    pub fn schema_version(&self) -> &str {
        &self.schema_version
    }

    /// Returns the experiment identifier.
    #[must_use]
    pub fn experiment_id(&self) -> &str {
        &self.experiment_id
    }

    /// Returns the internally derived certificate status.
    #[must_use]
    pub const fn status(&self) -> CertificateStatus {
        self.status
    }

    /// Returns the complete typed gate summary used to derive `status`.
    #[must_use]
    pub const fn gates(&self) -> CertificateGates {
        self.gates
    }

    /// Returns the execution policy bound to the derived status.
    #[must_use]
    pub const fn mode(&self) -> ExecutionMode {
        self.mode
    }

    /// Returns the rendered Markdown.
    #[must_use]
    pub fn markdown(&self) -> &str {
        &self.markdown
    }
}

/// Renders a ledger-first report. The first heading is always the status.
#[must_use]
pub fn render_narrative(
    experiment_id: &str,
    gates: &CertificateGates,
    ledger: &EvidenceLedger,
    extra_sections: &[(&str, String)],
) -> NarrativeReport {
    let status = ledger.status(gates);
    let mut markdown = String::new();
    let _ = writeln!(
        markdown,
        "# Mechanism Interferometry report\n\n## Certificate status: `{}`\n",
        status_label(status)
    );
    let _ = writeln!(
        markdown,
        "This report opens with assumptions and abstentions. It does not open with a table of significance statistics.\n"
    );
    let _ = writeln!(markdown, "## Assumptions\n");
    if ledger.provenance_fields().is_empty() {
        let _ = writeln!(markdown, "- No provenance fields were recorded.\n");
    } else {
        for (key, value) in ledger.provenance_fields() {
            let _ = writeln!(markdown, "- `{key}`: `{value}`");
        }
        markdown.push('\n');
    }
    let abstentions = findings_with(ledger, Severity::Error);
    let warnings = findings_with(ledger, Severity::Warning);
    let _ = writeln!(markdown, "## Abstentions\n");
    if abstentions.is_empty() {
        let _ = writeln!(
            markdown,
            "- No blocking reason codes. A certificate still requires locality, unique deletion orientation, and square flatness together.\n"
        );
    } else {
        for finding in abstentions {
            write_finding(&mut markdown, finding);
        }
        markdown.push('\n');
    }
    let _ = writeln!(markdown, "## Warnings\n");
    if warnings.is_empty() {
        let _ = writeln!(markdown, "- None.\n");
    } else {
        for finding in warnings {
            write_finding(&mut markdown, finding);
        }
        markdown.push('\n');
    }
    for (title, body) in extra_sections {
        let _ = writeln!(markdown, "## {title}\n\n{body}\n");
    }
    let _ = writeln!(markdown, "## Informational findings\n");
    let infos = findings_with(ledger, Severity::Info);
    if infos.is_empty() {
        let _ = writeln!(markdown, "- None.\n");
    } else {
        for finding in infos {
            write_finding(&mut markdown, finding);
        }
        markdown.push('\n');
    }
    NarrativeReport {
        schema_version: "2.0.0".into(),
        experiment_id: experiment_id.to_string(),
        status,
        gates: *gates,
        mode: ledger.mode(),
        markdown,
    }
}

fn findings_with(ledger: &EvidenceLedger, severity: Severity) -> Vec<&Finding> {
    ledger
        .findings()
        .iter()
        .filter(|finding| finding.severity == severity)
        .collect()
}

fn write_finding(markdown: &mut String, finding: &Finding) {
    let severity = match finding.severity {
        Severity::Info => "info",
        Severity::Warning => "warning",
        Severity::Error => "error",
    };
    let _ = writeln!(
        markdown,
        "- `{}` ({}/{}): {}",
        finding.code, finding.stage, severity, finding.message
    );
}

fn status_label(status: CertificateStatus) -> &'static str {
    match status {
        CertificateStatus::Passed => "passed",
        CertificateStatus::Failed => "failed",
        CertificateStatus::Abstained => "abstained",
        CertificateStatus::DiagnosticOnly => "diagnostic_only",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{CertificateGates, EvidenceLedger, ExecutionMode};

    #[test]
    fn markdown_starts_with_status_and_abstentions() {
        let mut ledger = EvidenceLedger::new(ExecutionMode::Strict);
        ledger.provenance("seed", "1");
        ledger.note(
            Severity::Error,
            "orientation",
            "orientation_unresolved",
            "multiple deletions are certified invariant",
        );
        let gates = CertificateGates::unresolved();
        let report = render_narrative("demo", &gates, &ledger, &[]);
        let first_heading = report
            .markdown()
            .lines()
            .find(|line| line.starts_with("## "))
            .unwrap();
        assert_eq!(first_heading, "## Certificate status: `abstained`");
        assert!(report.markdown().contains("## Abstentions"));
        assert!(
            report.markdown().find("## Abstentions").unwrap()
                < report.markdown().find("## Informational findings").unwrap()
        );
        assert!(!report.markdown().to_ascii_lowercase().contains("p-value"));
        assert_eq!(report.gates(), gates);
        assert_eq!(report.status(), ledger.status(&gates));
    }
}
