const defalutKeyInfo = {
  UP: "E",
  DOWN: "D",
  RIGHT: "S",
  LEFT: "F",
  A: "K",
  B: "J",
  SELECT: "Space",
  START: "Return"
}

const keyInfoString = localStorage.getItem("keyInfo");
console.log(keyInfoString);
const keyInfo = keyInfoString != null ? JSON.parse(keyInfoString) : defalutKeyInfo;

const makeKeyTable = (keyInfoArr) => {
  console.log(keyInfoArr);
  const keyTableBody = Object.keys(keyInfoArr).map((el) => {
    return (
      `<tr>
        <td> ${el} </td>
        <td> ${keyInfoArr[el]} </td>
        <td align="right"> <button type="button" class="btn btn-secondary btn-sm" id="key-config-${el}" data-bs-toggle="modal" data-bs-target="#keyModal"> 変更する </button> </td>
      </tr>`
    )
  });

  return keyTableBody.join("\n");
}

const tableElements = makeKeyTable(keyInfo);

document.querySelector("tbody").innerHTML = tableElements;

let selected = "";

Object.keys(keyInfo).forEach((el) => {
  document.getElementById(`key-config-${el}`).addEventListener('click', () => {
    console.log(el);
    selected = el;
  });
})

let keyData = {
  key: "",
  code: "",
  onShift: false,
  onCtrl: false,
  onAlt: false,
  onMeta: false
}

window.addEventListener("keydown", (e) => {
  keyData.key = e.key;
  keyData.code = e.code.replace('Key', '').replace('Arrow', '').replace('Digit', 'Key').replace('Enter', 'Return').replace('Backspace', 'Back');
  keyData.onShift = e.shiftKey;
  keyData.onCtrl = e.ctrlKey;
  keyData.onAlt = e.altKey;
  keyData.onMeta = e.metaKey;

  document.getElementById('key-input').value = keyData.code;
});

document.getElementById("close-button").addEventListener('click', () => {
  selected = "";
});

document.getElementById("confirm-button").addEventListener('click', () => {
  console.log(selected);
  if (selected !== "") keyInfo[selected] = keyData.code;
  const tableElements = makeKeyTable(keyInfo);
  document.querySelector("tbody").innerHTML = tableElements;
  Object.keys(keyInfo).forEach((el) => {
    document.getElementById(`key-config-${el}`).addEventListener('click', () => {
      selected = el;
    });
  })

  localStorage.setItem("keyInfo", JSON.stringify(keyInfo));
  document.getElementById('key-input').value = "";
  selected = "";
});
