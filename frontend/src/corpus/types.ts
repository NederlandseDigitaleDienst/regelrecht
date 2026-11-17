import type { LawDocument } from '../lib/lawFormat';

/** A traject's URL-form ref (`{slug}-{8hex}`), or null for the global corpus. */
export type TrajectRef = string | null;

/** `CorpusLawEntry` in packages/editor-api/src/corpus_handlers.rs. */
export interface CorpusLaw {
  law_id: string;
  name?: string | null;
  display_name?: string | null;
  source_id?: string;
  source_name?: string;
  source_priority?: number;
  /** The current version carries `machine_readable` (see the backend field). */
  enriched?: boolean;
  /** When the current version is not enriched but another is: that version's date. */
  enriched_version?: string | null;
  /** The law's folder on GitHub, for a law from a GitHub source. */
  source_url?: string | null;
}

/** An output a rulework offers to other ruleworks (GET .../outputs). */
export interface LawOutput {
  name: string;
  output_type?: string;
  article_number?: string;
  parameters?: { name: string; param_type?: string }[] | null;
}

/** A rulework's current version as the reader hands it out. */
export interface LawBody {
  law: LawDocument | null;
  rawYaml: string;
  lawName: string;
  /** Echoed back as `If-Match` on the next save. */
  etag: string | null;
}

/** Every version of a rulework, newest first, and the traject they came from. */
export interface LawVersions {
  trajectRef: TrajectRef;
  yamls: string[];
}

/** The filters of the law list route; see the editor-api docs for their meaning. */
export interface LawListQuery {
  /** Search: every word must occur in the id or the name. */
  q?: string;
  /** Exact ids. */
  ids?: readonly string[];
  limit?: number;
  /** Only laws whose body is loaded (the ones that carry a name). */
  loaded?: boolean;
}

/** One page of the law list, with the number of matches before the limit. */
export interface LawListPage {
  laws: CorpusLaw[];
  total: number;
}
