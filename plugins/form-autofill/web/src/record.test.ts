// RB-090-78: what `apply` names to a running recording per field it set - the target exactly as
// the sync agent names the field a person types into (src/target.js is a copy of the agent's
// selector), what was done, the value set, and never a password's value.
import { describe, expect, it } from 'vitest';
import { centreTarget } from '../../../../agent/src/selector';
import { page } from './support';

const FORM = `<form id="signup">
  <label>Email <input type="email" name="email"></label>
  <input type="text" name="nick" data-testid="nick">
  <input type="password" id="pw" name="pw">
  <label for="c">Country</label><select id="c" name="country"><option value="">-</option><option value="de">Germany</option></select>
  <div class="row"><input type="checkbox" name="terms" aria-label="I agree"></div>
  <div class="row"><input type="checkbox" name="news" aria-label="Newsletter" checked></div>
  <fieldset><legend>Pay</legend><input type="radio" name="pay" value="card"><input type="radio" name="pay" value="cash"></fieldset>
  <textarea name="bio" placeholder="About you"></textarea>
  <div><span><input name="loose"></span></div>
</form>`;

// eslint-disable-next-line @typescript-eslint/no-explicit-any -- the page script's answer
function fill(): { fields: any[]; elements: Element[] } {
  document.body.innerHTML = FORM;
  // eslint-disable-next-line @typescript-eslint/no-explicit-any -- the page script's answer
  const byName = new Map<string, any>(page({ op: 'collect' }).fields.map((field: any) => [field.name, field]));
  const at = (name: string) => byName.get(name);
  const text = (name: string, value: string) => ({ index: at(name).index, sig: at(name).sig, op: 'text', candidates: [value], goal: 'any' });
  const answer = page({ op: 'apply', actions: [
    text('email', 'ada@example.com'),
    text('nick', 'ada'),
    text('pw', 'Tr0ub4dor&3'),
    { index: at('country').index, sig: at('country').sig, op: 'select', values: ['de'], goal: 'valid' },
    { index: at('terms').index, sig: at('terms').sig, op: 'check', checked: true },
    { index: at('news').index, sig: at('news').sig, op: 'check', checked: false },
    { index: at('pay').index, sig: at('pay').sig, op: 'radio', value: 'cash' },
    text('bio', 'Hello'),
    text('loose', 'x'),
  ] });
  return { fields: answer.fields, elements: Array.from(document.querySelectorAll('input, select, textarea')) };
}

describe('apply names each field it set to the recorder', () => {
  it('with what was done and the value set, a password only marked', () => {
    const { fields } = fill();
    const records = fields.map((field) => field.record);
    expect(records.map((record) => record.kind)).toEqual(['fill', 'fill', 'fill', 'select', 'check', 'uncheck', 'check', 'fill', 'fill']);
    expect(records[0].value).toBe('ada@example.com');
    expect(records[2].sensitive).toBe(true);
    expect('value' in records[2]).toBe(false);
    expect(JSON.stringify(fields.map((field) => field.record))).not.toContain('Tr0ub4dor');
    expect(records[3]).toMatchObject({ value: 'de', selected: [1] });
    expect(records[1].target.element).toMatchObject({ kind: 'testid', testid: 'nick' });
    expect(records[4].target.element).toMatchObject({ kind: 'role', role: 'checkbox', name: 'I agree' });
    expect(records[6].target.element).toMatchObject({ kind: 'css' });
    expect(records[6].target.element.path).toContain('nth-of-type(2)');
  });

  it('names every field as the sync agent names the field a person types into', () => {
    const { fields, elements } = fill();
    for (const field of fields) {
      const element = elements[field.index] as HTMLInputElement;
      // A radio group is reported by its first radio; the record names the one checked.
      const named = element.type === 'radio' ? (document.querySelector('input[name="pay"]:checked') as Element) : element;
      expect(field.record.target, element.name).toEqual(centreTarget(named));
    }
  });

  it('leaves out a hidden input and a radio group left empty', () => {
    document.body.innerHTML = '<input type="hidden" name="h"><input type="radio" name="r" value="a" checked><input type="radio" name="r" value="b">';
    // eslint-disable-next-line @typescript-eslint/no-explicit-any -- the page script's answer
    const [hidden, group] = page({ op: 'collect', hiddenInputs: true }).fields as any[];
    const answer = page({ op: 'apply', hiddenInputs: true, actions: [
      { index: hidden.index, sig: hidden.sig, op: 'text', candidates: ['token'], goal: 'any' },
      { index: group.index, sig: group.sig, op: 'radio', value: null },
    ] });
    expect(answer.fields.map((field: { record: unknown }) => field.record)).toEqual([null, null]);
  });
});
