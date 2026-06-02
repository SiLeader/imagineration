# POST `/v1/images:generate`

指定したモデル・プロンプト・入力画像等を使って画像生成を実行します。

画像生成バックエンドには [stable-diffusion.cpp](https://github.com/leejet/stable-diffusion.cpp) の Rust バインディングである [diffusion-rs](https://github.com/newfla/diffusion-rs) を使用します。モデルのアーキテクチャ (SD1.x / SD2.x / SDXL / SD3.x / Flux / Qwen-Image / Z-Image / Anima など) は読み込んだ重みから自動判別されるため、リクエストでモデルファミリを指定する必要はありません。

生成された画像 (PNG) と、入力情報を含むメタデータがサーバーの `[paths] images_dir` (既定値 `data/images`) に保存されます。生成画像の PNG には、入力情報がテキストチャンク (キーワード `imagineration.metadata`) としてメタデータ埋め込みされます。

モデルファイルの探索ルートとなるディレクトリ構成については [directory-structure.md](../directory-structure.md) を参照してください。

## リクエスト

### ヘッダー

| 名前 | 値 |
| --- | --- |
| `Content-Type` | `application/json` |

### ボディ

リクエストボディは JSON オブジェクトを受け付けます。モデル、プロンプト、ステップ数、CFG など、画像生成に必要な情報を含めます。

ボディ全体はそのままメタデータの `request` フィールドとして保存されます。サーバーが特別に解釈するフィールドは以下のとおりです。

#### モデル指定

モデルは文字列パスで指定します。すべて `[paths] models_dir` (既定値 `models`) を基準に解決され、ファイル名のみ (パス区切りを含まない) の場合は、各フィールドに対応する既定サブディレクトリ配下を参照します。相対パスは `models_dir` からの相対パスとして解決します。`.safetensors`, `.ckpt`, `.gguf` (GGUF 量子化モデル), `.pt` を読み込めます。

フルチェックポイント (`model`) を指定するか、構成要素 (`diffusion_model` + テキストエンコーダ / LLM + `vae`) を個別に指定するか、diffusion-rs のプリセット (`preset`) を指定するかのいずれかを選択します。Flux、SD3、Qwen-Image、Z-Image-Turbo、Anima のように構成要素が分かれて配布されるモデルでは分割構成またはプリセットを使用します。

| フィールド | 型 | 既定のサブディレクトリ | 説明 |
| --- | --- | --- | --- |
| `preset` | string | — | diffusion-rs のプリセットを使用します。必要な重みは diffusion-rs が Hugging Face Hub から取得します。例: `qwen_image`, `z_image_turbo`, `anima`, `flux_2_dev`, `ovis_image`, `chroma`。 |
| `preset_weight_type` | string | — | プリセットの量子化型。未指定時は diffusion-rs 側の既定値です。例: `q2_k`, `q4_k`, `q4_k_m`, `q8_0`, `bf16`, `fp8_e4m3fn`。 |
| `model` | string | `checkpoints/` | UNet・VAE・テキストエンコーダを同梱したフルチェックポイント。これ単体で生成できます。 |
| `diffusion_model` | string | `diffusion_models/` | 分割構成の拡散モデル (UNet / DiT) 本体。テキストエンコーダと `vae` を併せて指定します。 |
| `high_noise_diffusion_model` | string | `diffusion_models/` | high-noise 側の拡散モデル。対応するモデルでのみ使用します。 |
| `vae` | string | `vae/` | VAE。`model` に VAE が含まれていても、ここで上書きできます。 |
| `taesd` | string | `vae/` | Tiny AutoEncoder。高速デコード用 (品質は低下します)。 |
| `text_encoders` | string[] | `text_encoders/` | テキストエンコーダをまとめて指定します。1〜4要素を受け付け、ファイル名から `clip_l` / `clip_g` / `t5xxl` / `llm` を推論します。推論できない場合は順に `clip_l`, `clip_g`, `t5xxl`, `llm` として扱います。 |
| `clip_l`, `clip_g`, `t5xxl` | string | `text_encoders/` | CLIP-L / CLIP-G / T5-XXL を個別に指定します。`text_encoders` と併用できますが、同じ役割を重複指定すると `400 Bad Request` になります。 |
| `llm` | string | `text_encoders/` | Qwen-Image、Z-Image-Turbo、Anima、Flux.2 系などで使う Qwen / Mistral などの LLM テキストエンコーダ。 |
| `llm_vision` | string | `clip_vision/` | Qwen2-VL / Qwen3-VL などで必要になる Vision 側エンコーダ。 |
| `clip_vision` | string | `clip_vision/` | CLIP-Vision エンコーダ。 |
| `control_net` | string | `controlnet/` | ControlNet モデル。 |
| `upscale_model` | string | `upscale_models/` | ESRGAN 系アップスケーラ。生成後に適用します。 |
| `photo_maker` | string | `photomaker/` | PhotoMaker モデル。 |
| `pm_id_embed_path` | string | `photomaker/` | PhotoMaker v2 の ID embedding。 |
| `weight_type` | string | — | 重みの型を明示的に上書きします (未指定時は重みファイルの型を使用)。`f32`, `f16`, `bf16`, `q8_0`, `q4_0`, `q4_1`, `q4_k`, `q5_0`, `q5_1`, `q5_k`, `q6_k`, `q2_k`, `q3_k`, `q8_k`, `iq4_nl`, `mxfp4` などの量子化型を受け付けます。 |

配列指定ではファイル名に `clip_l`, `clip_g`, `t5xxl`, `qwen`, `mistral`, `ovis`, `llm` が含まれる場合はその役割に割り当てられます。`qwen`, `mistral`, `ovis`, `llm` を含むファイルは `llm` として扱われます。名前から推論できない場合、1要素なら `clip_l`、2要素なら SDXL と同じ `clip_l`, `clip_g`、3要素なら `clip_l`, `clip_g`, `t5xxl`、4要素なら `clip_l`, `clip_g`, `t5xxl`, `llm` の順で割り当てられます。

#### 生成パラメータ

| フィールド | 型 | 既定値 | 説明 |
| --- | --- | --- | --- |
| `prompt` | string | なし | 生成プロンプト。必須。 |
| `negative_prompt` | string | `""` | ネガティブプロンプト。 |
| `width` | integer | `512` | 出力画像の幅 (ピクセル)。`1`〜`4096` の範囲かつ `8` の倍数。 |
| `height` | integer | `512` | 出力画像の高さ (ピクセル)。`1`〜`4096` の範囲かつ `8` の倍数。 |
| `steps` | integer | `20` | サンプリングステップ数。 |
| `cfg_scale` | number | `7.0` | CFG (classifier-free guidance) scale。 |
| `guidance` | number | `3.5` | distilled guidance scale。Flux などの guidance 入力を持つモデルで使用します。 |
| `sampling_method` | string | 自動 | サンプラー。未指定時は diffusion-rs がモデルに応じて選択します。後述の一覧を参照。 |
| `scheduler` | string | 自動 | ノイズスケジュール。未指定時は diffusion-rs がモデルとサンプラーに応じて選択します。後述の一覧を参照。 |
| `seed` | integer | `-1` | 生成シード。負数の場合はランダムシードになります。 |
| `batch_count` | integer | `1` | 生成枚数。 |
| `clip_skip` | integer | `-1` | CLIP の末尾から無視する層数。`-1` でモデル既定。 |
| `eta` | number | `0` | DDIM / TCD / res_multistep / res_2s でのみ使用する eta。 |
| `vae_tiling` | boolean | `false` | VAE をタイル分割して処理し、メモリ使用量を削減します。 |
| `flash_attention` | boolean | `false` | flash attention を使用し、メモリ使用量を削減します。 |
| `flow_shift` | number | なし | SD3.x や WAN などの flow 系モデルの shift 値。 |
| `timestep_shift` | integer | `0` | NitroFusion 系モデルなどの timestep shift。 |
| `prediction` | string | 自動 | 予測タイプの明示指定。`eps`, `v`, `edm_v`, `sd3_flow`, `flux_flow`, `flux2_flow`。 |
| `n_threads` | integer | 物理コア数 | CPU 計算スレッド数。`0` は物理コア数。 |
| `offload_params_to_cpu` | boolean | `false` | 重みを CPU RAM に置き、必要時に転送します。低 VRAM 環境向けです。 |
| `vae_on_cpu` | boolean | `false` | VAE を CPU に保持します。 |
| `clip_on_cpu` | boolean | `false` | テキストエンコーダを CPU に保持します。 |
| `control_net_cpu` | boolean | `false` | ControlNet を CPU に保持します。 |
| `diffusion_flash_attention` | boolean | `false` | 拡散モデル部分だけ flash attention を有効化します。 |
| `qwen_image_zero_cond_true` | boolean | `false` | Qwen-Image の zero condition true 最適化を有効化します。 |
| `chroma_disable_dit_mask` | boolean | `false` | Chroma 系モデルの DiT mask を無効化します。 |
| `chroma_enable_t5_mask` | boolean | `false` | Chroma 系モデルの T5 mask を有効化します。 |
| `chroma_t5_mask_pad` | integer | `1` | Chroma 系モデルの T5 mask padding。 |
| `diffusion_conv_direct` | boolean | `false` | 拡散モデルの Conv2D direct 実行を有効化します。 |
| `vae_conv_direct` | boolean | `false` | VAE の Conv2D direct 実行を有効化します。 |
| `force_sdxl_vae_conv_scale` | boolean | `false` | SDXL VAE の conv scale を強制します。 |
| `taesd_preview_only` | boolean | `false` | TAESD をプレビュー用途のみに制限します。 |
| `circular`, `circular_x`, `circular_y` | boolean | `false` | circular padding / RoPE wrapping を有効化します。 |
| `rng`, `sampler_rng` (`sampler_rng_type`) | string | `cuda`, `rng` と同じ | RNG 実装。`std_default`, `cuda`, `cpu`。 |
| `loras` | object[] | `[]` | 適用する LoRA。各要素は `file_name` と `weight` を持つオブジェクトです。 |
| `lora_apply_mode` | string | `auto` | LoRA の適用方式。`auto`, `immediately`, `at_runtime`。 |

#### サンプラー (`sampling_method`)

`euler`, `euler_a`, `heun`, `dpm2`, `dpmpp2s_a`, `dpmpp2m`, `dpmpp2mv2`, `ipndm`, `ipndm_v`, `lcm`, `ddim_trailing`, `tcd`, `res_multistep`, `res_2s`

#### スケジューラ (`scheduler`)

`discrete`, `karras`, `exponential`, `ays`, `gits`, `sgm_uniform`, `simple`, `smoothstep`, `kl_optimal`, `lcm`, `bong_tangent`

#### LoRA

LoRA は `loras` フィールドで指定します。`loras/` ディレクトリ配下のファイル名と重みを配列で指定し、0個以上の LoRA を適用できます。ファイル名は拡張子あり/なしのどちらでも指定できます。

```json
{
  "loras": [
    {
      "file_name": "detail-tweaker.safetensors",
      "weight": 0.8
    }
  ]
}
```

#### Embeddings (Textual Inversion)

Embedding はプロンプト内でファイル名 (トリガーワード) を記述することで適用されます。`embeddings/` ディレクトリ配下のファイルが参照されます。

#### 入力画像 (img2img / inpaint)

入力画像は `data:{mime};base64,{data}` 形式の data URL 文字列で指定します。

| フィールド | 型 | 説明 |
| --- | --- | --- |
| `init_image` | string (data URL) | img2img の初期画像。指定すると img2img として動作します。 |
| `mask_image` | string (data URL) | inpaint 用マスク。`init_image` と併せて指定します。白が再生成領域です。 |
| `strength` | number | img2img のノイズ付与強度。既定値 `0.75`。`0` に近いほど入力画像を保持します。 |
| `control_strength` | number | ControlNet の適用強度。既定値 `0.9`。 |
| `control_image` | string (data URL) | ControlNet の条件画像。`control_net` と併せて指定します。 |
| `ref_images` | string[] (data URL) | Flux.2 などの in-context conditioning 用参照画像。 |

#### 追加のバックエンドパラメータ

stable-diffusion.cpp / diffusion-rs の対応モデルで必要になる場合に、以下のフィールドも指定できます。

| フィールド | 型 | 既定値 | 説明 |
| --- | --- | --- | --- |
| `enable_mmap` | boolean | `false` | モデルファイルを memory-map します。 |
| `vae_tile_size` | integer[2] | `[32, 32]` | VAE tiling のタイルサイズ。 |
| `vae_relative_tile_size` | number[2] | `[0, 0]` | 画像サイズに対する VAE tiling の相対タイルサイズ。 |
| `vae_tile_overlap` | number | `0.5` | VAE tiling の重なり率。 |
| `upscale_repeats` | integer | `1` | `upscale_model` の適用回数。`0` で無効化。 |
| `upscale_tile_size` | integer | `128` | upscaler のタイルサイズ。 |
| `sigmas` | number[] | なし | サンプラーに渡すカスタム sigma 値。 |
| `slg_scale` | number | `0` | DiT モデル向け Skip Layer Guidance scale。 |
| `skip_layer` | integer[] | `[7, 8, 9]` | SLG の対象レイヤー。 |
| `skip_layer_start` | number | `0.01` | SLG の開始位置。 |
| `skip_layer_end` | number | `0.2` | SLG の終了位置。 |
| `canny` | boolean | `false` | Canny preprocessor を有効化します。 |
| `disable_auto_resize_ref_image` | boolean | `false` | 参照画像の自動リサイズを無効化します。 |
| `pm_style_strength` | number | `20.0` | PhotoMaker の style strength。 |

- Base64 エンコードされた data URL のみサポートします (`;base64` を含まない data URL は `400 Bad Request`)。
- data URL のパースに失敗した場合は `400 Bad Request` を返します。
- JSON のどの階層に置かれていても、`data:` で始まる文字列はすべて入力アセットとして検出され、メタデータの `input_assets` に記録されます。

#### リクエスト例

フルチェックポイント (SD1.5 / SDXL):

```json
{
  "model": "example.safetensors",
  "prompt": "a cat sitting on a chair",
  "negative_prompt": "blurry, low quality",
  "steps": 20,
  "loras": [
    {
      "file_name": "detail-tweaker.safetensors",
      "weight": 0.8
    }
  ],
  "cfg_scale": 7.0,
  "sampling_method": "dpmpp2m",
  "scheduler": "karras",
  "width": 768,
  "height": 512
}
```

分割構成 (Flux など):

```json
{
  "diffusion_model": "flux1-dev-q8_0.gguf",
  "text_encoders": [
    "clip_l.safetensors",
    "t5xxl_fp16.safetensors"
  ],
  "vae": "ae.safetensors",
  "prompt": "a cat sitting on a chair",
  "steps": 20,
  "cfg_scale": 1.0,
  "guidance": 3.5,
  "sampling_method": "euler",
  "width": 1024,
  "height": 1024
}
```

diffusion-rs プリセット (Qwen-Image / Z-Image-Turbo / Anima など):

```json
{
  "preset": "qwen_image",
  "preset_weight_type": "q2_k",
  "prompt": "a cat sitting on a chair"
}
```

`preset` を使う場合、steps、画像サイズ、CFG、VAE tiling、flash attention、offload などは diffusion-rs プリセットの既定値が使われます。リクエストで同じフィールドを指定すると上書きできます。

分割構成 (Qwen-Image):

```json
{
  "diffusion_model": "qwen_image_bf16.safetensors",
  "llm": "qwen_2.5_vl_7b.safetensors",
  "vae": "qwen_image_vae.safetensors",
  "prompt": "a cat sitting on a chair",
  "steps": 20,
  "cfg_scale": 2.5,
  "flow_shift": 3.0,
  "sampling_method": "euler",
  "flash_attention": true,
  "offload_params_to_cpu": true,
  "vae_tiling": true,
  "width": 1024,
  "height": 1024
}
```

分割構成 (Z-Image-Turbo):

```json
{
  "diffusion_model": "z_image_turbo-Q4_K.gguf",
  "llm": "Qwen3-4B-Instruct-2507-Q4_K_M.gguf",
  "vae": "diffusion_pytorch_model.safetensors",
  "prompt": "a cat sitting on a chair",
  "steps": 9,
  "cfg_scale": 1.0,
  "flash_attention": true,
  "vae_tiling": true,
  "width": 512,
  "height": 1024
}
```

分割構成 (Anima):

```json
{
  "diffusion_model": "anima-preview-Q4_K_M.gguf",
  "llm": "Qwen3-0.6B-Base.Q4_K_M.gguf",
  "vae": "qwen_image_vae.safetensors",
  "prompt": "anime portrait, detailed eyes",
  "steps": 30,
  "cfg_scale": 4.0,
  "vae_tiling": true,
  "width": 1024,
  "height": 1024
}
```

分割構成 (SDXL):

```json
{
  "diffusion_model": "sdxl-unet.safetensors",
  "text_encoders": [
    "clip_l.safetensors",
    "clip_g.safetensors"
  ],
  "vae": "sdxl-vae.safetensors",
  "prompt": "a cat sitting on a chair",
  "steps": 20,
  "cfg_scale": 7.0,
  "sampling_method": "dpmpp2m",
  "scheduler": "karras",
  "width": 1024,
  "height": 1024
}
```

img2img:

```json
{
  "model": "example.safetensors",
  "init_image": "data:image/png;base64,iVBORw0KGgo...",
  "strength": 0.6,
  "prompt": "a cat, oil painting",
  "steps": 20,
  "cfg_scale": 7.0,
  "width": 512,
  "height": 512
}
```

## レスポンス

### `201 Created`

`batch_count` 枚の画像が生成され、それぞれが個別の ID・メタデータとして保存されます。

```json
{
  "images": [
    {
      "id": "f47ac10b-58cc-7372-a567-0e02b2c3d479",
      "mime_type": "image/png",
      "image_url": "/v1/images/f47ac10b-58cc-7372-a567-0e02b2c3d479",
      "metadata_url": "/v1/images/f47ac10b-58cc-7372-a567-0e02b2c3d479/metadata",
      "created_at": "2026-05-29T12:00:00Z"
    }
  ]
}
```

#### フィールド

| フィールド | 型 | 説明 |
| --- | --- | --- |
| `images` | array | 生成された画像の配列。生成順に並びます。 |
| `images[].id` | string (UUID v7) | 生成された画像の ID。 |
| `images[].mime_type` | string | 出力画像の MIME タイプ。常に `image/png`。 |
| `images[].image_url` | string | 画像本体を取得するための相対 URL。 |
| `images[].metadata_url` | string | メタデータを取得するための相対 URL。 |
| `images[].created_at` | string (RFC 3339) | 生成日時 (UTC)。 |

### エラー

| ステータス | 発生条件 |
| --- | --- |
| `400 Bad Request` | 必須フィールド不足、モデル指定不足、`width` / `height` が範囲外または `8` の倍数でない、不正な data URL、未知のサンプラー / スケジューラ / `weight_type`、ボディが JSON として不正。 |
| `404 Not Found` | 指定したモデルファイルが `models_dir` に存在しない。 |
| `500 Internal Server Error` | generator backend、画像・メタデータの書き込み、PNG メタデータ埋め込みの失敗。 |

## 保存されるメタデータ

生成時に `images_dir/{id}.json` として保存され、[GET `/v1/images/{image_id}/metadata`](./get-image-metadata.md) で取得できる内容と同一です。

```json
{
  "id": "f47ac10b-58cc-7372-a567-0e02b2c3d479",
  "created_at": "2026-05-29T12:00:00Z",
  "request": { "...": "リクエストボディそのまま" },
  "input_assets": [
    {
      "json_pointer": "/init_image",
      "mime_type": "image/png",
      "size_bytes": 5
    }
  ],
  "output": {
    "mime_type": "image/png",
    "width": 768,
    "height": 512,
    "image_path": "f47ac10b-58cc-7372-a567-0e02b2c3d479.png"
  }
}
```

## 例

```bash
curl -X POST http://127.0.0.1:3000/v1/images:generate \
  -H 'Content-Type: application/json' \
  -d '{
    "model": "example.safetensors",
    "prompt": "a cat",
    "steps": 20,
    "cfg_scale": 7.0,
    "sampling_method": "euler_a",
    "scheduler": "karras",
    "width": 512,
    "height": 512
  }'
```
