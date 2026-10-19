//! Utilitários de versão (semver simples, sem dependências).

pub type Triple = (u64, u64, u64);

/// `1.2.3` -> Some; pré-releases (`1.2.3-beta.1`) e formatos estranhos -> None.
pub fn parse_stable(v: &str) -> Option<Triple> {
    let mut it = v.split('.');
    let a = it.next()?.parse().ok()?;
    let b = it.next()?.parse().ok()?;
    let c = it.next()?.parse().ok()?;
    if it.next().is_some() {
        return None;
    }
    Some((a, b, c))
}

/// Extrai a versão base de um especificador (`^4.1.2`, `~5`, `>=1.2`), preenchendo com zeros.
pub fn clean_spec(spec: &str) -> Option<Triple> {
    let s = spec.trim().trim_start_matches(|c: char| !c.is_ascii_digit());
    let end = s.find(|c: char| !(c.is_ascii_digit() || c == '.')).unwrap_or(s.len());
    let mut parts = s[..end].split('.').filter(|p| !p.is_empty()).map(|p| p.parse::<u64>());
    let a = parts.next()?.ok()?;
    let b = parts.next().map_or(Some(0), |r| r.ok())?;
    let c = parts.next().map_or(Some(0), |r| r.ok())?;
    Some((a, b, c))
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MajorChoice {
    pub major: u64,
    pub version: String,
    pub is_latest: bool,
}

/// A versão estável mais recente de cada major acima da atual (até `max`), da mais nova para a mais antiga.
pub fn major_choices(all: &[String], current: Option<Triple>, max: usize) -> Vec<MajorChoice> {
    let mut stable: Vec<(Triple, &String)> = all
        .iter()
        .filter_map(|v| parse_stable(v).map(|t| (t, v)))
        .filter(|(t, _)| current.is_none_or(|c| *t > c))
        .collect();
    stable.sort_by_key(|b| std::cmp::Reverse(b.0));
    let mut out: Vec<MajorChoice> = vec![];
    for ((major, _, _), v) in stable {
        if out.iter().any(|m| m.major == major) {
            continue;
        }
        out.push(MajorChoice { major, version: v.clone(), is_latest: out.is_empty() });
        if out.len() >= max {
            break;
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn v(list: &[&str]) -> Vec<String> {
        list.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn cleans_specs() {
        assert_eq!(clean_spec("^4.1.2"), Some((4, 1, 2)));
        assert_eq!(clean_spec("~5"), Some((5, 0, 0)));
        assert_eq!(clean_spec(">=1.2 <2"), Some((1, 2, 0)));
        assert_eq!(clean_spec("latest"), None);
    }

    #[test]
    fn picks_newest_per_major() {
        let all = v(&["17.0.2", "18.2.0", "18.3.1", "19.0.0", "19.1.0", "19.2.0", "19.3.0-beta.1"]);
        let c = major_choices(&all, Some((17, 0, 2)), 4);
        let got: Vec<_> = c.iter().map(|m| (m.major, m.version.as_str(), m.is_latest)).collect();
        assert_eq!(got, vec![(19, "19.2.0", true), (18, "18.3.1", false)]);
        let c = major_choices(&all, Some((19, 1, 0)), 4);
        assert_eq!(c.len(), 1);
        assert_eq!(c[0].version, "19.2.0");
        assert!(major_choices(&all, Some((19, 2, 0)), 4).is_empty());
    }
}
