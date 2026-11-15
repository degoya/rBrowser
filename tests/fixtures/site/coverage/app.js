// The Coverage fixture's script (RB-080-07): `countClick` runs only on a click of the button,
// `neverCalled` never runs.
function countClick() {
  const output = document.getElementById('clicks');
  output.textContent = String(Number(output.textContent) + 1);
}

function neverCalled() {
  return 'never';
}

document.getElementById('count').addEventListener('click', countClick);
