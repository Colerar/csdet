# csdet

**C**har**s**et **Det**ection CLI.

## Build

```bash
cargo build --relase
```

## Usage

```bash
Charset Detection CLI

Usage: csdet [OPTIONS] [FILES]...

Arguments:
  [FILES]...  

Options:
      --buf <BUF>                  [default: 8192]
      --preview-buf <PREVIEW_BUF>  [default: 128]
      --limit <LIMIT>              [default: 16384]
  -c, --confirm                    
  -h, --help                       Print help
  -V, --version                    Print version
```

For example, all input files will be converted to UTF-8 encoding:

```plaintext
❯ cat *.txt | head
�����Ă��������v
�΂��茩�Ă��
�������܂ɂ���΂ق�
�܂��n�܂��
���񂾂����i�ގ��Ԃ�
�����l������邩��
���݂������߂�������
��̓r����
���������ȓ����܂肪
�������ł��Ă�

❯ csdet *.txt
[00:00:00] ########################################       1/1       All files are detected!
╭──────────────────────┬───────────┬───────────────────────────╮
│ File                 │ Encoding  │ Preview (first 128 bytes) │
╞══════════════════════╪═══════════╪═══════════════════════════╡
│ some_legacy_text.txt │ Shift_JIS │ 落ちていく砂時計 　　　　　　　│
│                      │           │ ばかり見てるよ 　　　　　　　　│
│                      │           │ さかさまにすればほら 　　　　　│
│                      │           │ また始まるよ 　　　　　　　　　│
│                      │           │ 刻んだだけ進む時間に 　　　　　│
│                      │           │  　　　　　　　　　　　　　　　│
╰──────────────────────┴───────────┴───────────────────────────╯
[00:00:00] ########################################       1/1       All files are converted!

❯ cat *.txt | head
落ちていく砂時計
ばかり見てるよ
さかさまにすればほら
また始まるよ
刻んだだけ進む時間に
いつか僕も入れるかな
きみだけが過ぎ去った
坂の途中は
あたたかな日だまりが
いくつもできてた
```
