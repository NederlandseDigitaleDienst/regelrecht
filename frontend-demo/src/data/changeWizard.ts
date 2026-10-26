/**
 * Wijziging doorgeven: één plek waar iemand een verandering in zijn leven
 * meldt, in plaats van per regeling een waarde te corrigeren.
 *
 * De POC had hiervoor een wizard op het dashboard (`FEATURE_CHANGE_WIZARD`,
 * web/templates/partials/dashboard.html) die naar `/edit/update-situation`
 * postte. Die endpoint deed maar één ding: de antwoorden vertalen naar
 * correcties (claims) op de wet die het gegeven bezit — inkomen naar de
 * Wet inkomstenbelasting, adres en huishouden naar de Wet BRP, huur naar de
 * Wet op de huurtoeslag. Dat is precies wat de demo zelf al kan
 * (`demoStore.submitClaim`), dus hier staat alleen die vertaling.
 *
 * Wat het toevoegt boven het corrigeren van een losse waarde: iemand denkt in
 * gebeurtenissen ("ik ben verhuisd", "mijn inkomen is veranderd"), niet in
 * velden van een register. Eén melding raakt meerdere waarden tegelijk, en
 * elke regeling die ervan afhangt rekent meteen opnieuw.
 *
 * Alles wat op het scherm komt staat hier als sleutel, niet als zin: de
 * structuur is in beide talen dezelfde en `typeLabel()`, `fieldLabel()` en
 * hun buren zijn de enige plek die er een taal aan geeft. Hetzelfde patroon
 * als `simulation/stats.ts` volgt voor zijn dimensies.
 */
import { t } from '../i18n/index.js';

/** One field of the form. */
export interface ChangeField {
  /** The input name on the law. */
  name: string;
  labelKey: string;
  /** Multiplier from what the form asks to what the register holds. */
  scale?: number;
  kind?: 'text';
  /** The composite input this field is part of. */
  part?: string;
}

/** A group of fields under one heading. */
export interface ChangeGroup {
  labelKey: string;
  fields: ChangeField[];
}

/** A life event that changes several values at once. */
export interface ChangeEvent {
  value: string;
  labelKey: string;
  icon: string;
  /** input name -> new value */
  changes?: Record<string, unknown>;
  /** Set when the demo cannot handle this event yet. */
  unsupportedKey?: string;
}

/** A kind of change: the law that owns the data and the fields or events it takes. */
export interface ChangeType {
  id: string;
  labelKey: string;
  icon: string;
  descriptionKey: string;
  law: string;
  keyField: string;
  groups?: ChangeGroup[];
  events?: ChangeEvent[];
}

/** A correction to submit, as `claimsFromAnswers` returns it. */
export interface ChangeClaim {
  law: string;
  input: string;
  value: unknown;
  label: string;
}

/** The answers in the form: field name (or `event`) -> what was entered. */
export type ChangeAnswers = Record<string, unknown>;

/** Bedragen staan in de registers in centen; het formulier vraagt hele euro's. */
const EUROS = { scale: 100 };

/**
 * De soorten wijziging, elk met de wet die de gegevens bezit en de velden die
 * meegaan. `law` is de wet-id waarop de correctie landt, `keyField` de sleutel
 * waarop die wet haar gegevens opzoekt.
 *
 * Een veld zonder waarde wordt niet ingediend: leeglaten betekent
 * ongewijzigd, niet nul.
 */
export const CHANGE_TYPES: ChangeType[] = [
  {
    id: 'inkomen',
    labelKey: 'sheet.change.type.inkomen',
    icon: 'euro-sign',
    descriptionKey: 'sheet.change.type.inkomen.description',
    law: 'wet_inkomstenbelasting',
    keyField: 'bsn',
    groups: [
      {
        labelKey: 'sheet.change.group.box1',
        fields: [
          { name: 'loon_uit_dienstbetrekking', labelKey: 'sheet.change.field.loon_uit_dienstbetrekking', ...EUROS },
          { name: 'uitkeringen_en_pensioenen', labelKey: 'sheet.change.field.uitkeringen_en_pensioenen', ...EUROS },
          { name: 'winst_uit_onderneming', labelKey: 'sheet.change.field.winst_uit_onderneming', ...EUROS },
          { name: 'resultaat_overige_werkzaamheden', labelKey: 'sheet.change.field.resultaat_overige_werkzaamheden', ...EUROS },
          { name: 'eigen_woning', labelKey: 'sheet.change.field.eigen_woning', ...EUROS },
        ],
      },
      {
        labelKey: 'sheet.change.group.box2',
        fields: [
          { name: 'reguliere_voordelen', labelKey: 'sheet.change.field.reguliere_voordelen', ...EUROS },
          { name: 'vervreemdingsvoordelen', labelKey: 'sheet.change.field.vervreemdingsvoordelen', ...EUROS },
        ],
      },
      {
        labelKey: 'sheet.change.group.box3',
        fields: [
          { name: 'spaargeld', labelKey: 'sheet.change.field.spaargeld', ...EUROS },
          { name: 'beleggingen', labelKey: 'sheet.change.field.beleggingen', ...EUROS },
          { name: 'onroerend_goed', labelKey: 'sheet.change.field.onroerend_goed', ...EUROS },
          { name: 'schulden', labelKey: 'sheet.change.field.schulden', ...EUROS },
        ],
      },
    ],
  },
  {
    id: 'huurprijs',
    labelKey: 'sheet.change.type.huurprijs',
    icon: 'house',
    descriptionKey: 'sheet.change.type.huurprijs.description',
    law: 'wet_op_de_huurtoeslag',
    keyField: 'bsn',
    groups: [
      {
        labelKey: 'sheet.change.group.monthly',
        fields: [
          { name: 'huurprijs', labelKey: 'sheet.change.field.huurprijs', ...EUROS },
          { name: 'servicekosten', labelKey: 'sheet.change.field.servicekosten', ...EUROS },
          { name: 'subsidiabele_servicekosten', labelKey: 'sheet.change.field.subsidiabele_servicekosten', ...EUROS },
        ],
      },
    ],
  },
  {
    id: 'woonadres',
    labelKey: 'sheet.change.type.woonadres',
    icon: 'location',
    descriptionKey: 'sheet.change.type.woonadres.description',
    law: 'wet_brp',
    keyField: 'bsn',
    groups: [
      {
        labelKey: 'sheet.change.group.new_address',
        fields: [
          { name: 'straat', labelKey: 'sheet.change.field.straat', kind: 'text', part: 'adres' },
          { name: 'huisnummer', labelKey: 'sheet.change.field.huisnummer', kind: 'text', part: 'adres' },
          { name: 'postcode', labelKey: 'sheet.change.field.postcode', kind: 'text', part: 'adres' },
          { name: 'woonplaats', labelKey: 'sheet.change.field.woonplaats', kind: 'text', part: 'adres' },
        ],
      },
    ],
  },
  {
    id: 'huishouden',
    labelKey: 'sheet.change.type.huishouden',
    icon: 'person-2',
    descriptionKey: 'sheet.change.type.huishouden.description',
    law: 'wet_brp',
    keyField: 'bsn',
    // Een gebeurtenis, geen bedrag: de keuze bepaalt wat er verandert.
    events: [
      {
        value: 'scheiden',
        labelKey: 'sheet.change.event.scheiden',
        icon: 'person-badge-minus',
        // De POC ondersteunde alleen deze; de andere kwamen terug als
        // "Deze flow is nog niet ondersteund". Hier gebeurt hetzelfde,
        // maar dan zichtbaar vóór het indienen in plaats van erna.
        changes: { partnerschap_type: 'GEEN', partner_bsn: null },
      },
      {
        value: 'samenwonen',
        labelKey: 'sheet.change.event.samenwonen',
        icon: 'heart',
        unsupportedKey: 'sheet.change.event.samenwonen.unsupported',
      },
      { value: 'kind', labelKey: 'sheet.change.event.kind', icon: 'person-badge-plus', unsupportedKey: 'sheet.change.event.kind.unsupported' },
      { value: 'iemand-bij', labelKey: 'sheet.change.event.iemand_bij', icon: 'person-badge-plus', unsupportedKey: 'sheet.change.event.iemand_bij.unsupported' },
      { value: 'iemand-weg', labelKey: 'sheet.change.event.iemand_weg', icon: 'person-badge-minus', unsupportedKey: 'sheet.change.event.iemand_weg.unsupported' },
      { value: 'overlijden', labelKey: 'sheet.change.event.overlijden', icon: 'person-badge-minus', unsupportedKey: 'sheet.change.event.overlijden.unsupported' },
    ],
  },
];

export function changeTypeById(id: string | null | undefined): ChangeType | null {
  return CHANGE_TYPES.find((t) => t.id === id) ?? null;
}

/** Alle velden van een soort wijziging, plat. */
export function fieldsOf(type: ChangeType | null | undefined): ChangeField[] {
  return (type?.groups ?? []).flatMap((g) => g.fields);
}

/** Hoe een soort wijziging, een groep, een veld of een gebeurtenis heet op het scherm. */
export function typeLabel(type: ChangeType | null | undefined): string {
  return type?.labelKey ? t(type.labelKey) : '';
}

export function typeDescription(type: ChangeType | null | undefined): string {
  return type?.descriptionKey ? t(type.descriptionKey) : '';
}

export function groupLabel(group: ChangeGroup | null | undefined): string {
  return group?.labelKey ? t(group.labelKey) : '';
}

export function fieldLabel(field: ChangeField | null | undefined): string {
  return field?.labelKey ? t(field.labelKey) : '';
}

export function eventLabel(event: ChangeEvent | null | undefined): string {
  return event?.labelKey ? t(event.labelKey) : '';
}

/** Waarom een gebeurtenis in deze demo nog niet kan; lege tekst als zij wél kan. */
export function eventUnsupported(event: ChangeEvent | null | undefined): string {
  return event?.unsupportedKey ? t(event.unsupportedKey) : '';
}

/**
 * Een ingevuld formulier omzetten naar de correcties die ingediend worden.
 *
 * Leeg betekent ongewijzigd, dus die velden vallen weg. Een bedrag gaat als
 * centen naar de wet, want zo staat het in het register. Het adres is één
 * samengestelde waarde (`adres`, kind: record in bindings.yaml); de wet maakt
 * er zelf de adresregel (`verblijfsadres`) van.
 *
 * `label` is de vertaalde tekst op het moment van indienen: de bevestiging
 * toont haar meteen en verder gaat zij als reden mee naar de correctie.
 */
export function claimsFromAnswers(type: ChangeType | null | undefined, answers: ChangeAnswers | null | undefined): ChangeClaim[] {
  if (!type) return [];
  const filled = (name: string): boolean => {
    const v = answers?.[name];
    return v !== undefined && v !== null && String(v).trim() !== '';
  };

  if (type.id === 'huishouden') {
    const event = (type.events ?? []).find((e) => e.value === answers?.event);
    if (!event || event.unsupportedKey || !event.changes) return [];
    return Object.entries(event.changes).map(([input, value]) => ({
      law: type.law,
      input,
      value,
      label: eventLabel(event),
    }));
  }

  if (type.id === 'woonadres') {
    const parts = fieldsOf(type).filter((f) => filled(f.name));
    if (!parts.length) return [];
    const adres = Object.fromEntries(parts.map((f) => [f.name, String(answers?.[f.name]).trim()]));
    return [{ law: type.law, input: 'adres', value: { ...adres, type: 'WOONADRES' }, label: t('sheet.change.field.adres') }];
  }

  return fieldsOf(type)
    .filter((f) => filled(f.name))
    .map((f) => {
      const raw = Number(String(answers?.[f.name]).replace(',', '.'));
      return {
        law: type.law,
        input: f.name,
        value: f.scale ? Math.round(raw * f.scale) : raw,
        label: fieldLabel(f),
      };
    })
    .filter((c) => Number.isFinite(c.value));
}
