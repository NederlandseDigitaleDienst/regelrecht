import { describe, it, expect } from 'vitest';
import { buildTypeMap, buildExternalFieldTypeMap, type FieldDeclaration } from './articleMapping';

describe('buildTypeMap', () => {
  it('maps parameter and input names to their declared type', () => {
    const articles = [
      {
        number: '2',
        machine_readable: {
          execution: {
            parameters: [{ name: 'bsn', type: 'string' }],
            input: [{ name: 'is_verzekerde', type: 'boolean' }],
          },
        },
      },
    ];
    const map = buildTypeMap(articles);
    expect(map.get('bsn')).toEqual({ type: 'string', unit: null, nullable: false });
    expect(map.get('is_verzekerde')).toEqual({ type: 'boolean', unit: null, nullable: false });
  });

  it('captures type_spec.unit for amounts', () => {
    const articles = [
      {
        number: '3',
        machine_readable: {
          execution: {
            parameters: [{ name: 'inkomen', type: 'amount', type_spec: { unit: 'eurocent' } }],
          },
        },
      },
    ];
    expect(buildTypeMap(articles).get('inkomen')).toEqual({ type: 'amount', unit: 'eurocent', nullable: false });
  });

  it('lets parameter types override input types on name collision', () => {
    const articles = [
      {
        number: '1',
        machine_readable: {
          execution: {
            input: [{ name: 'x', type: 'string' }],
            parameters: [{ name: 'x', type: 'number' }],
          },
        },
      },
    ];
    expect(buildTypeMap(articles).get('x')).toEqual({ type: 'number', unit: null, nullable: false });
  });

  // RFC-036 / schema v0.5.8: `nullable` says whether `null` is a value of the
  // field. Absent means false (the schema default), and only the literal
  // `true` counts - a truthy string from an unchecked YAML is not a declaration.
  it('records nullable: true only for a field declared nullable', () => {
    const articles = [
      {
        number: '1',
        machine_readable: {
          execution: {
            parameters: [
              { name: 'huur', type: 'amount', nullable: true },
              { name: 'inkomen', type: 'amount' },
              { name: 'leeftijd', type: 'number', nullable: 'true' },
            ],
          },
        },
      },
    ];
    const map = buildTypeMap(articles);
    expect(map.get('huur')?.nullable).toBe(true);
    expect(map.get('inkomen')?.nullable).toBe(false);
    expect(map.get('leeftijd')?.nullable).toBe(false);
  });

  // Precedence on a name collision is the parameter's, for nullable as much
  // as for type: a parameter row of the form is checked by the engine against
  // the parameter's declaration (`required_parameter_for_nobody`), whatever
  // a same-named input says. Both directions, so neither is an accident of
  // pass order.
  describe('nullable on a parameter/input name collision', () => {
    const colliding = (input: FieldDeclaration, parameter: FieldDeclaration) => [
      { number: '1', machine_readable: { execution: { input: [input] } } },
      { number: '2', machine_readable: { execution: { parameters: [parameter] } } },
    ] as const;

    it('a non-nullable parameter shadows a nullable input: null is refused for the parameter row', () => {
      const map = buildTypeMap(colliding(
        { name: 'partner_bsn', type: 'string', nullable: true, source: {} },
        { name: 'partner_bsn', type: 'string' },
      ));
      expect(map.get('partner_bsn')).toEqual({ type: 'string', unit: null, nullable: false });
    });

    it('a nullable parameter shadows a non-nullable input: null is accepted for the parameter row', () => {
      const map = buildTypeMap(colliding(
        { name: 'partner_bsn', type: 'string', source: {} },
        { name: 'partner_bsn', type: 'string', nullable: true },
      ));
      expect(map.get('partner_bsn')).toEqual({ type: 'string', unit: null, nullable: true });
    });

    it('the parameter wins regardless of article order', () => {
      const [inputArticle, paramArticle] = colliding(
        { name: 'x', type: 'amount', type_spec: { unit: 'eurocent' }, nullable: true, source: {} },
        { name: 'x', type: 'number' },
      );
      expect(buildTypeMap([paramArticle, inputArticle]).get('x')).toEqual({ type: 'number', unit: null, nullable: false });
      expect(buildTypeMap([inputArticle, paramArticle]).get('x')).toEqual({ type: 'number', unit: null, nullable: false });
    });

    it('an input with no same-named parameter keeps its own declaration', () => {
      const map = buildTypeMap([{ number: '1', machine_readable: { execution: { input: [{ name: 'huur', type: 'amount', nullable: true, source: {} }] } } }]);
      expect(map.get('huur')?.nullable).toBe(true);
    });
  });

  it('ignores articles without machine_readable and returns a Map', () => {
    const map = buildTypeMap([{ number: '1' }, {}]);
    expect(map).toBeInstanceOf(Map);
    expect(map.size).toBe(0);
  });

  it('handles empty / nullish input', () => {
    expect(buildTypeMap(undefined).size).toBe(0);
    expect(buildTypeMap([]).size).toBe(0);
  });
});

describe('buildExternalFieldTypeMap', () => {
  const law = (inputs: FieldDeclaration[]) => ({ articles: [{ machine_readable: { execution: { input: inputs } } }] });

  it('collects external (source:{}) inputs as name -> {type, unit}', () => {
    const m = buildExternalFieldTypeMap([law([
      { name: 'verdragsinschrijving', type: 'boolean', source: {} },
      { name: 'spaargeld', type: 'amount', type_spec: { unit: 'eurocent' }, source: {} },
    ])]);
    expect(m.get('verdragsinschrijving')).toEqual({ type: 'boolean', unit: null, nullable: false });
    expect(m.get('spaargeld')).toEqual({ type: 'amount', unit: 'eurocent', nullable: false });
  });

  it('records whether an external input is declared nullable', () => {
    const m = buildExternalFieldTypeMap([law([
      { name: 'huur', type: 'amount', nullable: true, source: {} },
      { name: 'spaargeld', type: 'amount', source: {} },
    ])]);
    expect(m.get('huur')?.nullable).toBe(true);
    expect(m.get('spaargeld')?.nullable).toBe(false);
  });

  // The other side of the precedence in buildTypeMap: a table column feeds an
  // input, and the engine checks it against the input's declaration, so a
  // same-named parameter, nullable or not, leaves the column's meta alone.
  it('is untouched by a same-named parameter, in either direction of nullability', () => {
    const withParam = (input: FieldDeclaration, parameter: FieldDeclaration) => ({
      articles: [{ machine_readable: { execution: { input: [input], parameters: [parameter] } } }],
    });
    const shadowedNullable = buildExternalFieldTypeMap([withParam(
      { name: 'partner_bsn', type: 'string', nullable: true, source: {} },
      { name: 'partner_bsn', type: 'number' },
    )]);
    expect(shadowedNullable.get('partner_bsn')).toEqual({ type: 'string', unit: null, nullable: true });
    const shadowedStrict = buildExternalFieldTypeMap([withParam(
      { name: 'partner_bsn', type: 'string', source: {} },
      { name: 'partner_bsn', type: 'string', nullable: true },
    )]);
    expect(shadowedStrict.get('partner_bsn')).toEqual({ type: 'string', unit: null, nullable: false });
  });

  it('excludes cross-law (source.regulation) and internal (source.output) inputs', () => {
    const m = buildExternalFieldTypeMap([law([
      { name: 'toetsingsinkomen', type: 'amount', source: { regulation: 'awir', output: 'x' } },
      { name: 'internal', type: 'number', source: { output: 'y' } },
    ])]);
    expect(m.has('toetsingsinkomen')).toBe(false);
    expect(m.has('internal')).toBe(false);
  });

  it('merges across multiple law docs; last doc wins on name collision', () => {
    const m = buildExternalFieldTypeMap([
      law([{ name: 'x', type: 'string', source: {} }]),
      law([{ name: 'x', type: 'boolean', source: {} }]),
    ]);
    expect(m.get('x')).toEqual({ type: 'boolean', unit: null, nullable: false });
  });

  it('tolerates empty / missing docs and articles without machine_readable', () => {
    expect(buildExternalFieldTypeMap(undefined).size).toBe(0);
    expect(buildExternalFieldTypeMap([null, { articles: [{}] }]).size).toBe(0);
  });
});
