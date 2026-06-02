# ディレクトリ構造

Imagineration サーバーが参照・出力するディレクトリの構成です。パスは設定ファイル (`imagineration.toml`) の `[paths]` セクションで指定します。

| 設定キー | 既定値 | 用途 |
| --- | --- | --- |
| `models_dir` | `models` | モデルファイルの探索ルート。 |
| `images_dir` | `data/images` | 生成画像とメタデータの保存先。 |

## モデルディレクトリ (`models_dir`)

モデルディレクトリは [ComfyUI](https://github.com/comfyanonymous/ComfyUI) の `models` ディレクトリと同様の構成です。種類ごとにサブディレクトリを設け、その配下にモデルファイルを配置します。サブディレクトリも含めて再帰的に走査されます。

[POST `/v1/images:generate`](./apis/post-images-generate.md) でモデルをファイル名のみで指定した場合、フィールドに対応する以下のサブディレクトリ配下が参照されます。バックエンドの [diffusion-rs](https://github.com/newfla/diffusion-rs) ([stable-diffusion.cpp](https://github.com/leejet/stable-diffusion.cpp) の Rust バインディング) は、これらの構成要素を組み合わせて画像生成を行います。

| ディレクトリ | 用途 | 対応するリクエストフィールド |
| --- | --- | --- |
| `checkpoints/` | UNet・VAE・テキストエンコーダを同梱したフルチェックポイント (SD1.x / SD2.x / SDXL / SD3.x など)。 | `model` |
| `diffusion_models/` | 分割構成の拡散モデル (UNet / DiT) 本体のみ (Flux / Qwen-Image / Z-Image / Anima など)。 | `diffusion_model`, `high_noise_diffusion_model` |
| `text_encoders/` | CLIP-L / CLIP-G / T5-XXL / Qwen / Mistral などのテキストエンコーダまたは LLM。 | `text_encoders`, `clip_l`, `clip_g`, `t5xxl`, `llm` |
| `vae/` | VAE および TAESD。 | `vae`, `taesd` |
| `loras/` | LoRA。`POST /v1/images:generate` の `loras` フィールドでファイル名と重みを指定して参照。 | `loras` |
| `embeddings/` | Textual Inversion 埋め込み。プロンプト内のトリガーワードで参照。 | (プロンプト構文) |
| `controlnet/` | ControlNet モデル。 | `control_net` |
| `clip_vision/` | CLIP-Vision / Qwen-VL Vision エンコーダ。 | `clip_vision`, `llm_vision` |
| `upscale_models/` | ESRGAN 系アップスケーラ。 | `upscale_model` |
| `photomaker/` | PhotoMaker モデルおよび ID embedding。 | `photo_maker`, `pm_id_embed_path` |

対応するモデルファイルの拡張子は `.safetensors`, `.ckpt`, `.gguf` (GGUF 量子化モデル), `.pt` です。

### 構成例

```text
models/
├── checkpoints/
│   ├── sd15-example.safetensors
│   └── sdxl-example.safetensors
├── diffusion_models/
│   ├── flux1-dev-q8_0.gguf
│   ├── qwen_image_bf16.safetensors
│   ├── z_image_turbo-Q4_K.gguf
│   └── anima-preview-Q4_K_M.gguf
├── text_encoders/
│   ├── clip_l.safetensors
│   ├── clip_g.safetensors
│   ├── t5xxl_fp16.safetensors
│   ├── qwen_2.5_vl_7b.safetensors
│   ├── Qwen3-4B-Instruct-2507-Q4_K_M.gguf
│   └── Mistral-Small-3.2-24B-Instruct-2506-Q4_K_M.gguf
├── vae/
│   ├── sdxl-vae.safetensors
│   ├── ae.safetensors
│   └── qwen_image_vae.safetensors
├── loras/
│   └── detail-tweaker.safetensors
├── embeddings/
│   └── easynegative.safetensors
├── controlnet/
│   └── control-canny.safetensors
├── clip_vision/
├── upscale_models/
│   └── RealESRGAN_x4plus.safetensors
└── photomaker/
```

利用可能なモデルの一覧は [GET `/v1/models`](./apis/get-models.md) で取得できます。

## 画像ディレクトリ (`images_dir`)

生成された画像とメタデータが保存されます。画像 ID (UUID v7) ごとに、PNG 画像と JSON メタデータが対になります。

```text
data/images/
├── f47ac10b-58cc-7372-a567-0e02b2c3d479.png
└── f47ac10b-58cc-7372-a567-0e02b2c3d479.json
```

- `{id}.png` … 生成画像本体。入力情報がテキストチャンク (キーワード `imagineration.metadata`) として埋め込まれます。
- `{id}.json` … 生成時の入力情報 (メタデータ)。詳細は [GET `/v1/images/{image_id}/metadata`](./apis/get-image-metadata.md) を参照してください。
