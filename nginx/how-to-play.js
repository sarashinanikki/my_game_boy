const defalutKeyInfo = [
  {
    button: "UP",
    key:    "E"
  },
  {
    button: "DOWN",
    key:    "D"
  },
  {
    button: "LEFT",
    key:    "S"
  },
  {
    button: "RIGHT",
    key:    "F"
  },
  {
    button: "A",
    key:    "K"
  },
  {
    button: "B",
    key:    "J"
  },
  {
    button: "SELECT",
    key:    "Space"
  },
  {
    button: "START",
    key:    "Enter"
  }
]

const keyInfoString = localStorage.getItem("keyInfo");
console.log(keyInfoString);
const keyInfo = keyInfoString != null ? JSON.parse(keyInfoString) : defalutKeyInfo;

const makeKeyTable = (keyInfoArr) => {
  const keyTableBody = keyInfoArr.map((el, idx) => {
    return (
      `<tr>
        <td> ${el.button} </td>
        <td> ${el.key}    </td>
        <td align="right"> <button type="button" class="btn btn-secondary btn-sm" id="key-config-${idx}" data-bs-toggle="modal" data-bs-target="#keyModal"> 変更する </button> </td>
      </tr>`
    )
  });

  return keyTableBody.join("\n");
}

const tableElements = makeKeyTable(keyInfo);

document.querySelector("tbody").innerHTML = tableElements;

let selected = -1;

keyInfo.forEach((_el, idx) => {
  document.getElementById(`key-config-${idx}`).addEventListener('click', () => {
    selected = idx;
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
  keyData.code = e.code;
  keyData.onShift = e.shiftKey;
  keyData.onCtrl = e.ctrlKey;
  keyData.onAlt = e.altKey;
  keyData.onMeta = e.metaKey;
  console.log(keyData.key);

  document.getElementById('key-input').value = keyData.key;
});

document.getElementById("close-button").addEventListener('click', () => {
  selected = -1;
});

document.getElementById("confirm-button").addEventListener('click', () => {
  console.log(selected);
  if (selected >= 0) keyInfo[selected].key = keyData.key;
  const tableElements = makeKeyTable(keyInfo);
  document.querySelector("tbody").innerHTML = tableElements;
  keyInfo.forEach((_el, idx) => {
    document.getElementById(`key-config-${idx}`).addEventListener('click', () => {
      selected = idx;
    });
  })

  localStorage.setItem("keyInfo", JSON.stringify(keyInfo));
  document.getElementById('key-input').value = "";
  selected = -1;
});
