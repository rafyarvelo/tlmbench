// Shortcuts read the way each platform writes them (R3.4 copy review): the app spells them the Mac way, and keyName turns
// them into Ctrl+, Alt+ and Shift+ on Windows and Linux. No browser.
import { keyName } from '../src/commands.ts';

let failed = 0;
let passed = 0;
const check = (label, cond, got) => {
  if (cond) passed++;
  else failed++;
  console.log(`  ${cond ? 'ok  ' : 'FAIL'} ${label}${cond || got === undefined ? '' : ' — got ' + JSON.stringify(got)}`);
};

const pc = (t) => keyName(t, false);
check('the Mac keeps its symbols', keyName('Save layout (⌘S)', true) === 'Save layout (⌘S)');
check('⌘ is Ctrl elsewhere', pc('Save layout (⌘S)') === 'Save layout (Ctrl+S)', pc('Save layout (⌘S)'));
check('modifiers in the usual order', pc('⌥⌘B') === 'Ctrl+Alt+B' && pc('⌘⇧N') === 'Ctrl+Shift+N' && pc('⇧⌘Z') === 'Ctrl+Shift+Z', [pc('⌥⌘B'), pc('⌘⇧N'), pc('⇧⌘Z')]);
check('a symbol key and ⌥ alone', pc('Back (⌥←)') === 'Back (Alt+←)' && pc('Focus mode ⌘.') === 'Focus mode Ctrl+.', [pc('Back (⌥←)'), pc('Focus mode ⌘.')]);
check('the sentence after the key is left alone', pc('or press ⌘O.') === 'or press Ctrl+O.', pc('or press ⌘O.'));
check('text without a shortcut is unchanged', pc('Fit to the data (F)') === 'Fit to the data (F)');

console.log(`${failed ? 'FAIL' : 'PASS'} keynames: ${passed} ok, ${failed} failed`);
process.exit(failed ? 1 : 0);
