<script>
  import { onMount } from 'svelte';

  // Compiled-in feature flag, forwarded from the `presets` cargo feature by build.rs.
  const presetsFeature = import.meta.env.VITE_PRESETS_ENABLED === 'true';
  const TOKEN_STORAGE_KEY = 'imagineration.token';

  const presetOptions = [
    'qwen_image',
    'z_image_turbo',
    'anima',
    'flux_2_dev',
    'ovis_image',
    'chroma'
  ];
  const samplerOptions = [
    '',
    'euler',
    'euler_a',
    'heun',
    'dpm2',
    'dpmpp2s_a',
    'dpmpp2m',
    'dpmpp2mv2',
    'ipndm',
    'ipndm_v',
    'lcm',
    'ddim_trailing',
    'tcd',
    'res_multistep',
    'res_2s'
  ];
  const schedulerOptions = [
    '',
    'discrete',
    'karras',
    'exponential',
    'ays',
    'gits',
    'sgm_uniform',
    'simple',
    'smoothstep',
    'kl_optimal',
    'lcm',
    'bong_tangent'
  ];
  const loraApplyModeOptions = ['auto', 'immediately', 'at_runtime'];

  let models = [];
  let images = [];
  let metadata = null;
  let selectedImage = null;
  let loadingModels = false;
  let loadingImages = false;
  let generating = false;
  let error = '';
  let notice = '';

  // Authentication state.
  let token = '';
  let capabilities = null;
  let authReady = false;
  let username = '';
  let password = '';
  let manualToken = '';
  let loginError = '';
  let loggingIn = false;

  // Object URLs for protected images, keyed by their original URL.
  let blobUrls = {};
  const blobPending = new Set();

  // User-defined presets.
  let presets = [];
  let presetName = '';
  let selectedPresetId = '';
  let loadingPresets = false;

  let mode = 'checkpoint';
  let model = '';
  let preset = 'qwen_image';
  let presetWeightType = '';
  let diffusionModel = '';
  let textEncoders = '';
  let vae = '';
  let prompt = '';
  let negativePrompt = '';
  let width = 768;
  let height = 768;
  let steps = 20;
  let cfgScale = 7;
  let guidance = 3.5;
  let seed = -1;
  let batchCount = 1;
  let samplingMethod = 'dpmpp2m';
  let scheduler = 'karras';
  let advancedJson = '';
  let checkpointModelFilter = '';
  let splitModelFilter = '';
  let loraFilter = '';
  let loraModel = '';
  let loraWeight = 1;
  let loraApplyMode = 'auto';
  let selectedLoras = [];

  $: checkpointModels = filterModels(models, 'checkpoints', checkpointModelFilter);
  $: diffusionModels = filterModels(models, 'diffusion_models', splitModelFilter);
  $: textEncoderModels = filterModels(models, 'text_encoders', splitModelFilter);
  $: vaeModels = filterModels(models, 'vae', splitModelFilter);
  $: loraModels = filterModels(models, 'loras', loraFilter);
  $: syncLoraModel(loraModels);
  $: visibleImages = images.slice(0, 24);
  $: requestPreview = previewRequest();
  $: needsLogin = authReady && capabilities?.auth?.required && !token;
  $: presetsEnabled = presetsFeature && capabilities?.presets === true;

  onMount(async () => {
    token = localStorage.getItem(TOKEN_STORAGE_KEY) || '';
    await loadCapabilities();
    if (!needsLogin) {
      await refresh();
    }
  });

  // --- Authentication -------------------------------------------------------

  function authHeaders(extra = {}) {
    return token ? { ...extra, Authorization: `Bearer ${token}` } : { ...extra };
  }

  // Computed directly (not via the reactive `presetsEnabled`) so callers see the current value
  // immediately after `capabilities` loads, before Svelte flushes reactive declarations.
  function arePresetsEnabled() {
    return presetsFeature && capabilities?.presets === true;
  }

  async function loadCapabilities() {
    try {
      const response = await fetch('/v1/capabilities');
      capabilities = response.ok ? await response.json() : null;
    } catch {
      capabilities = null;
    } finally {
      authReady = true;
    }
  }

  function persistToken(value) {
    token = value;
    if (value) {
      localStorage.setItem(TOKEN_STORAGE_KEY, value);
    } else {
      localStorage.removeItem(TOKEN_STORAGE_KEY);
    }
  }

  function clearToken() {
    persistToken('');
    revokeBlobs();
    models = [];
    images = [];
    presets = [];
    selectedImage = null;
    metadata = null;
  }

  async function login() {
    loginError = '';
    loggingIn = true;
    try {
      const response = await fetch('/v1/auth/login', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ username, password })
      });
      const data = await responseJson(response);
      if (!response.ok) {
        throw new Error(errorMessage(data, response.status));
      }
      password = '';
      persistToken(data.access_token);
      await refresh();
    } catch (caught) {
      loginError = caught.message;
    } finally {
      loggingIn = false;
    }
  }

  async function useManualToken() {
    loginError = '';
    if (!manualToken.trim()) {
      loginError = 'Enter a token';
      return;
    }
    persistToken(manualToken.trim());
    manualToken = '';
    await refresh();
  }

  function logout() {
    clearToken();
  }

  // --- Data loading ---------------------------------------------------------

  async function refresh() {
    error = '';
    const tasks = [loadModels(), loadImages()];
    if (arePresetsEnabled()) {
      tasks.push(loadPresets());
    }
    await Promise.all(tasks);
  }

  async function loadModels() {
    loadingModels = true;
    try {
      const data = await requestJson('/v1/models');
      models = data.models || [];
      selectDefaults();
    } catch (caught) {
      error = caught.message;
    } finally {
      loadingModels = false;
    }
  }

  async function loadImages() {
    loadingImages = true;
    try {
      const data = await requestJson('/v1/images');
      images = data.images || [];
      if (!selectedImage && images.length > 0) {
        await selectImage(images[0]);
      }
    } catch (caught) {
      error = caught.message;
    } finally {
      loadingImages = false;
    }
  }

  function selectDefaults() {
    const checkpoints = modelsByType('checkpoints');
    const diffusion = modelsByType('diffusion_models');
    const encoders = modelsByType('text_encoders');
    const vaes = modelsByType('vae');

    if (!model && checkpoints.length > 0) {
      model = modelName(checkpoints[0]);
    }
    if (!diffusionModel && diffusion.length > 0) {
      diffusionModel = modelName(diffusion[0]);
    }
    if (!vae && vaes.length > 0) {
      vae = modelName(vaes[0]);
    }
    if (!textEncoders && encoders.length > 0) {
      textEncoders = encoders.slice(0, 2).map(modelName).join('\n');
    }
    if (!loraModel) {
      loraModel = firstUnselectedLoraName();
    }
  }

  function modelsByType(type) {
    return models.filter((item) => modelType(item) === type);
  }

  function filterModels(source, type, filter) {
    const needle = filter.trim().toLowerCase();
    return source.filter((item) => {
      if (modelType(item) !== type) {
        return false;
      }
      return !needle || modelName(item).toLowerCase().includes(needle);
    });
  }

  function modelType(item) {
    return item.type || item.model_type || '';
  }

  function modelName(item) {
    return item.name || item.path || '';
  }

  function loraRequestFileName(value) {
    return value.split('/').pop() || value;
  }

  function firstUnselectedLoraName() {
    return modelsByType('loras').map(modelName).find((name) => !hasSelectedLora(name)) || '';
  }

  function hasSelectedLora(value) {
    const fileName = loraRequestFileName(value);
    return selectedLoras.some((item) => item.file_name === fileName);
  }

  function isValidLoraWeight(value) {
    return parseLoraWeight(value) !== null;
  }

  function parseLoraWeight(value) {
    if (value === '' || value === null || value === undefined) {
      return null;
    }
    const weight = Number(value);
    return Number.isFinite(weight) ? weight : null;
  }

  function syncLoraModel(visibleLoras) {
    if (visibleLoras.length === 0) {
      loraModel = '';
      return;
    }

    const visibleNames = visibleLoras.map(modelName);
    if (visibleNames.includes(loraModel)) {
      return;
    }

    loraModel = visibleNames.find((name) => !hasSelectedLora(name)) || visibleNames[0];
  }

  function addLora() {
    if (!loraModel || hasSelectedLora(loraModel) || !isValidLoraWeight(loraWeight)) {
      return;
    }
    selectedLoras = [
      ...selectedLoras,
      {
        name: loraModel,
        file_name: loraRequestFileName(loraModel),
        weight: parseLoraWeight(loraWeight)
      }
    ];
    loraModel = firstUnselectedLoraName();
  }

  function removeLora(index) {
    selectedLoras = selectedLoras.filter((_, itemIndex) => itemIndex !== index);
    if (!loraModel) {
      loraModel = firstUnselectedLoraName();
    }
  }

  function updateLoraWeight(index, value) {
    selectedLoras = selectedLoras.map((item, itemIndex) => {
      if (itemIndex !== index) {
        return item;
      }
      return { ...item, weight: value };
    });
  }

  async function generate() {
    error = '';
    notice = '';
    generating = true;
    try {
      const payload = buildRequest(true);
      const response = await fetch('/v1/images:generate', {
        method: 'POST',
        headers: authHeaders({ 'Content-Type': 'application/json' }),
        body: JSON.stringify(payload)
      });
      if (response.status === 401) {
        handleUnauthorized();
        return;
      }
      const data = await responseJson(response);
      if (!response.ok) {
        throw new Error(errorMessage(data, response.status));
      }
      notice = `${data.images?.length || 0} image generated`;
      await loadImages();
      if (data.images?.[0]) {
        await selectImage(data.images[0]);
      }
    } catch (caught) {
      error = caught.message;
    } finally {
      generating = false;
    }
  }

  async function selectImage(image) {
    selectedImage = image;
    metadata = null;
    try {
      metadata = await requestJson(image.metadata_url);
    } catch (caught) {
      error = caught.message;
    }
  }

  async function requestJson(url) {
    const response = await fetch(url, { headers: authHeaders() });
    if (response.status === 401) {
      handleUnauthorized();
      throw new Error('Authentication required');
    }
    const data = await responseJson(response);
    if (!response.ok) {
      throw new Error(errorMessage(data, response.status));
    }
    return data;
  }

  function handleUnauthorized() {
    error = 'Session expired. Please sign in again.';
    clearToken();
  }

  async function responseJson(response) {
    const text = await response.text();
    if (!text) {
      return {};
    }
    try {
      return JSON.parse(text);
    } catch {
      return { error: { message: text } };
    }
  }

  function errorMessage(data, status) {
    return data?.error?.message || `Request failed with ${status}`;
  }

  // --- Protected image loading ----------------------------------------------
  // <img> tags cannot carry a bearer token, so when authenticated we fetch the
  // bytes ourselves and hand the element a blob URL.

  function imageSource(url) {
    if (!url || !token) {
      return url;
    }
    if (blobUrls[url]) {
      return blobUrls[url];
    }
    loadBlob(url);
    return '';
  }

  async function loadBlob(url) {
    if (blobPending.has(url)) {
      return;
    }
    blobPending.add(url);
    try {
      const response = await fetch(url, { headers: authHeaders() });
      if (!response.ok) {
        return;
      }
      const blob = await response.blob();
      blobUrls = { ...blobUrls, [url]: URL.createObjectURL(blob) };
    } catch {
      // Leave the placeholder in place on failure.
    } finally {
      blobPending.delete(url);
    }
  }

  function revokeBlobs() {
    for (const url of Object.values(blobUrls)) {
      URL.revokeObjectURL(url);
    }
    blobUrls = {};
    blobPending.clear();
  }

  // --- Request building -----------------------------------------------------

  function previewRequest() {
    try {
      return JSON.stringify(buildRequest(false), null, 2);
    } catch (caught) {
      return caught.message;
    }
  }

  function buildRequest(validatePrompt) {
    const request = {};
    applyModelFields(request, validatePrompt);
    setString(request, 'prompt', prompt, validatePrompt);
    setString(request, 'negative_prompt', negativePrompt, false);
    setNumber(request, 'width', width);
    setNumber(request, 'height', height);
    setNumber(request, 'steps', steps);
    setNumber(request, 'cfg_scale', cfgScale);
    setNumber(request, 'guidance', guidance);
    setNumber(request, 'seed', seed);
    setNumber(request, 'batch_count', batchCount);
    setString(request, 'sampling_method', samplingMethod, false);
    setString(request, 'scheduler', scheduler, false);
    applyLoraFields(request);
    return { ...request, ...parseAdvancedJson() };
  }

  function applyLoraFields(request) {
    if (selectedLoras.length === 0) {
      return;
    }

    request.loras = selectedLoras.map((item) => {
      const weight = parseLoraWeight(item.weight);
      if (weight === null) {
        throw new Error('LoRA weight must be a number');
      }
      return {
        file_name: item.file_name,
        weight
      };
    });
    setString(request, 'lora_apply_mode', loraApplyMode, false);
  }

  function applyModelFields(request, validateFields) {
    if (mode === 'preset') {
      setString(request, 'preset', preset, validateFields);
      setString(request, 'preset_weight_type', presetWeightType, false);
      return;
    }
    if (mode === 'split') {
      setString(request, 'diffusion_model', diffusionModel, validateFields);
      const encoders = textEncoders.split('\n').map((value) => value.trim()).filter(Boolean);
      if (encoders.length > 0) {
        request.text_encoders = encoders;
      } else if (validateFields) {
        throw new Error('Text encoders are required');
      }
      setString(request, 'vae', vae, false);
      return;
    }
    setString(request, 'model', model, validateFields);
  }

  function setString(request, key, value, required) {
    const trimmed = value.trim();
    if (trimmed) {
      request[key] = trimmed;
    } else if (required) {
      throw new Error(`${key} is required`);
    }
  }

  function setNumber(request, key, value) {
    if (value === '' || value === null || value === undefined) {
      return;
    }
    const parsed = Number(value);
    if (Number.isFinite(parsed)) {
      request[key] = parsed;
    }
  }

  function parseAdvancedJson() {
    const source = advancedJson.trim();
    if (!source) {
      return {};
    }
    const parsed = JSON.parse(source);
    if (!parsed || typeof parsed !== 'object' || Array.isArray(parsed)) {
      throw new Error('Advanced JSON must be an object');
    }
    return parsed;
  }

  // --- User-defined presets -------------------------------------------------

  async function loadPresets() {
    if (!arePresetsEnabled()) {
      return;
    }
    loadingPresets = true;
    try {
      const data = await requestJson('/v1/presets');
      presets = data.presets || [];
    } catch (caught) {
      error = caught.message;
    } finally {
      loadingPresets = false;
    }
  }

  function buildPresetContent() {
    // The preset body mirrors the generation request, plus the UI mode hint.
    return { mode, ...buildRequest(false) };
  }

  async function savePreset() {
    error = '';
    notice = '';
    const name = presetName.trim();
    if (!name) {
      error = 'Preset name is required';
      return;
    }
    try {
      const response = await fetch('/v1/presets', {
        method: 'POST',
        headers: authHeaders({ 'Content-Type': 'application/json' }),
        body: JSON.stringify({ name, content: buildPresetContent() })
      });
      if (response.status === 401) {
        handleUnauthorized();
        return;
      }
      const data = await responseJson(response);
      if (!response.ok) {
        throw new Error(errorMessage(data, response.status));
      }
      presetName = '';
      notice = `Saved preset "${name}"`;
      await loadPresets();
      selectedPresetId = data.id;
    } catch (caught) {
      error = caught.message;
    }
  }

  function loadSelectedPreset() {
    const found = presets.find((item) => item.id === selectedPresetId);
    if (!found) {
      return;
    }
    applyPresetContent(found.content || {});
    notice = `Loaded preset "${found.name}"`;
  }

  async function deleteSelectedPreset() {
    if (!selectedPresetId) {
      return;
    }
    error = '';
    try {
      const response = await fetch(`/v1/presets/${selectedPresetId}`, {
        method: 'DELETE',
        headers: authHeaders()
      });
      if (response.status === 401) {
        handleUnauthorized();
        return;
      }
      if (!response.ok && response.status !== 404) {
        const data = await responseJson(response);
        throw new Error(errorMessage(data, response.status));
      }
      selectedPresetId = '';
      notice = 'Preset deleted';
      await loadPresets();
    } catch (caught) {
      error = caught.message;
    }
  }

  function applyPresetContent(content) {
    const rest = { ...content };
    const take = (key) => {
      const value = rest[key];
      delete rest[key];
      return value;
    };

    mode = take('mode') || (content.preset ? 'preset' : content.diffusion_model ? 'split' : 'checkpoint');
    if ('model' in rest) model = take('model') || '';
    if ('diffusion_model' in rest) diffusionModel = take('diffusion_model') || '';
    if ('text_encoders' in rest) {
      const encoders = take('text_encoders');
      textEncoders = Array.isArray(encoders) ? encoders.join('\n') : (encoders || '');
    }
    if ('vae' in rest) vae = take('vae') || '';
    if ('preset' in rest) preset = take('preset') || preset;
    if ('preset_weight_type' in rest) presetWeightType = take('preset_weight_type') || '';
    if ('loras' in rest) {
      const loras = take('loras');
      selectedLoras = Array.isArray(loras)
        ? loras.map((item) => ({ name: item.file_name, file_name: item.file_name, weight: item.weight }))
        : [];
    }
    if ('lora_apply_mode' in rest) loraApplyMode = take('lora_apply_mode') || 'auto';
    if ('prompt' in rest) prompt = take('prompt') || '';
    if ('negative_prompt' in rest) negativePrompt = take('negative_prompt') || '';
    if ('width' in rest) width = take('width');
    if ('height' in rest) height = take('height');
    if ('steps' in rest) steps = take('steps');
    if ('cfg_scale' in rest) cfgScale = take('cfg_scale');
    if ('guidance' in rest) guidance = take('guidance');
    if ('seed' in rest) seed = take('seed');
    if ('batch_count' in rest) batchCount = take('batch_count');
    if ('sampling_method' in rest) samplingMethod = take('sampling_method') || '';
    if ('scheduler' in rest) scheduler = take('scheduler') || '';

    // Anything else round-trips through the advanced JSON box.
    advancedJson = Object.keys(rest).length ? JSON.stringify(rest, null, 2) : '';
  }

  function formatDate(value) {
    return new Intl.DateTimeFormat(undefined, {
      dateStyle: 'medium',
      timeStyle: 'short'
    }).format(new Date(value));
  }
</script>

<svelte:head>
  <title>Imagineration</title>
</svelte:head>

{#if needsLogin}
  <div class="auth-shell">
    <section class="auth-card" aria-label="Sign in">
      <h1>Imagineration</h1>
      <p class="subtle">Authentication is required to use this server.</p>

      {#if capabilities?.auth?.login}
        <form on:submit|preventDefault={login}>
          <label>
            <span>Username</span>
            <input bind:value={username} autocomplete="username" />
          </label>
          <label>
            <span>Password</span>
            <input type="password" bind:value={password} autocomplete="current-password" />
          </label>
          <button type="submit" class="primary" disabled={loggingIn || !username}>
            {loggingIn ? 'Signing in' : 'Sign in'}
          </button>
        </form>
        <div class="auth-divider"><span>or</span></div>
      {/if}

      <form on:submit|preventDefault={useManualToken}>
        <label>
          <span>Bearer token</span>
          <input
            bind:value={manualToken}
            placeholder="Paste a token (e.g. from your IdP)"
            autocomplete="off"
          />
        </label>
        <button type="submit" class="secondary" disabled={!manualToken.trim()}>Use token</button>
      </form>

      {#if loginError}
        <p class="error">{loginError}</p>
      {/if}
    </section>
  </div>
{:else}
  <div class="shell">
    <header class="topbar">
      <div>
        <h1>Imagineration</h1>
        <div class="subtle">{models.length} models · {images.length} images</div>
      </div>
      <div class="status-line">
        {#if error}
          <span class="error">{error}</span>
        {:else if notice}
          <span class="notice">{notice}</span>
        {:else if generating}
          <span>Generating</span>
        {:else if loadingModels || loadingImages}
          <span>Loading</span>
        {:else}
          <span>Ready</span>
        {/if}
        <button type="button" class="secondary" on:click={refresh} disabled={loadingModels || loadingImages || generating}>
          Refresh
        </button>
        {#if token}
          <button type="button" class="secondary" on:click={logout}>Sign out</button>
        {/if}
      </div>
    </header>

    <main class="workspace">
      <section class="panel composer" aria-label="Generate">
        <div class="panel-head">
          <h2>Generate</h2>
          <div class="segments" role="tablist" aria-label="Model mode">
            <button type="button" class:active={mode === 'checkpoint'} on:click={() => (mode = 'checkpoint')}>
              Checkpoint
            </button>
            <button type="button" class:active={mode === 'split'} on:click={() => (mode = 'split')}>
              Split
            </button>
            <button type="button" class:active={mode === 'preset'} on:click={() => (mode = 'preset')}>
              Preset
            </button>
          </div>
        </div>

        {#if presetsEnabled}
          <div class="presets-box">
            <div class="presets-head">
              <h3>My presets</h3>
              <button type="button" class="secondary" on:click={loadPresets} disabled={loadingPresets}>
                Reload
              </button>
            </div>
            <div class="presets-row">
              <select bind:value={selectedPresetId}>
                <option value="">Select a preset…</option>
                {#each presets as item}
                  <option value={item.id}>{item.name}</option>
                {/each}
              </select>
              <button type="button" class="secondary" on:click={loadSelectedPreset} disabled={!selectedPresetId}>
                Load
              </button>
              <button type="button" class="secondary" on:click={deleteSelectedPreset} disabled={!selectedPresetId}>
                Delete
              </button>
            </div>
            <div class="presets-row save">
              <input bind:value={presetName} placeholder="New preset name" autocomplete="off" />
              <button type="button" class="secondary" on:click={savePreset} disabled={!presetName.trim()}>
                Save current
              </button>
            </div>
          </div>
        {/if}

        <form on:submit|preventDefault={generate}>
          {#if mode === 'checkpoint'}
            <label>
              <span>Checkpoint filter</span>
              <input bind:value={checkpointModelFilter} autocomplete="off" />
            </label>
            <label>
              <span>Checkpoint</span>
              <select bind:value={model}>
                {#each checkpointModels as item}
                  <option value={modelName(item)}>{modelName(item)}</option>
                {/each}
              </select>
            </label>
          {:else if mode === 'split'}
            <label>
              <span>Split model filter</span>
              <input bind:value={splitModelFilter} autocomplete="off" />
            </label>
            <label>
              <span>Diffusion model</span>
              <select bind:value={diffusionModel}>
                {#each diffusionModels as item}
                  <option value={modelName(item)}>{modelName(item)}</option>
                {/each}
              </select>
            </label>
            <label>
              <span>Text encoders</span>
              <textarea bind:value={textEncoders} rows="3"></textarea>
            </label>
            <label>
              <span>VAE</span>
              <select bind:value={vae}>
                <option value="">Auto</option>
                {#each vaeModels as item}
                  <option value={modelName(item)}>{modelName(item)}</option>
                {/each}
              </select>
            </label>
          {:else}
            <div class="two-col">
              <label>
                <span>Preset</span>
                <select bind:value={preset}>
                  {#each presetOptions as item}
                    <option value={item}>{item}</option>
                  {/each}
                </select>
              </label>
              <label>
                <span>Weight type</span>
                <input bind:value={presetWeightType} autocomplete="off" />
              </label>
            </div>
          {/if}

          <div class="lora-box">
            <div class="lora-head">
              <h3>LoRA</h3>
              <label>
                <span>Apply mode</span>
                <select bind:value={loraApplyMode}>
                  {#each loraApplyModeOptions as item}
                    <option value={item}>{item}</option>
                  {/each}
                </select>
              </label>
            </div>

            <label>
              <span>LoRA filter</span>
              <input bind:value={loraFilter} autocomplete="off" />
            </label>

            <div class="lora-add">
              <label>
                <span>LoRA</span>
                <select bind:value={loraModel} disabled={loraModels.length === 0}>
                  {#each loraModels as item}
                    <option value={modelName(item)} disabled={hasSelectedLora(modelName(item))}>
                      {modelName(item)}
                    </option>
                  {/each}
                </select>
              </label>
              <label>
                <span>Weight</span>
                <input type="number" step="0.05" bind:value={loraWeight} />
              </label>
              <button
                type="button"
                class="secondary add-lora"
                on:click={addLora}
                disabled={!loraModel || hasSelectedLora(loraModel) || !isValidLoraWeight(loraWeight)}
              >
                Add
              </button>
            </div>

            {#if selectedLoras.length > 0}
              <div class="selected-loras">
                {#each selectedLoras as item, index}
                  <div class="selected-lora">
                    <div>
                      <strong>{item.name}</strong>
                      <span>{item.file_name}</span>
                    </div>
                    <input
                      aria-label={`Weight for ${item.name}`}
                      type="number"
                      step="0.05"
                      value={item.weight}
                      on:input={(event) => updateLoraWeight(index, event.currentTarget.value)}
                    />
                    <button type="button" class="secondary" on:click={() => removeLora(index)}>
                      Remove
                    </button>
                  </div>
                {/each}
              </div>
            {/if}
          </div>

          <label>
            <span>Prompt</span>
            <textarea bind:value={prompt} rows="5"></textarea>
          </label>
          <label>
            <span>Negative prompt</span>
            <textarea bind:value={negativePrompt} rows="3"></textarea>
          </label>

          <div class="grid-fields">
            <label>
              <span>Width</span>
              <input type="number" min="8" max="4096" step="8" bind:value={width} />
            </label>
            <label>
              <span>Height</span>
              <input type="number" min="8" max="4096" step="8" bind:value={height} />
            </label>
            <label>
              <span>Steps</span>
              <input type="number" min="1" max="10000" bind:value={steps} />
            </label>
            <label>
              <span>Batch</span>
              <input type="number" min="1" max="256" bind:value={batchCount} />
            </label>
            <label>
              <span>CFG</span>
              <input type="number" min="0" step="0.1" bind:value={cfgScale} />
            </label>
            <label>
              <span>Guidance</span>
              <input type="number" min="0" step="0.1" bind:value={guidance} />
            </label>
            <label>
              <span>Seed</span>
              <input type="number" bind:value={seed} />
            </label>
            <label>
              <span>Sampler</span>
              <select bind:value={samplingMethod}>
                {#each samplerOptions as item}
                  <option value={item}>{item || 'Auto'}</option>
                {/each}
              </select>
            </label>
            <label>
              <span>Scheduler</span>
              <select bind:value={scheduler}>
                {#each schedulerOptions as item}
                  <option value={item}>{item || 'Auto'}</option>
                {/each}
              </select>
            </label>
          </div>

          <label>
            <span>Advanced JSON</span>
            <textarea bind:value={advancedJson} rows="6" spellcheck="false"></textarea>
          </label>

          <button type="submit" class="primary" disabled={generating}>
            {generating ? 'Generating' : 'Generate'}
          </button>
        </form>
      </section>

      <section class="panel request-panel" aria-label="Request">
        <div class="panel-head">
          <h2>Request</h2>
        </div>
        <pre>{requestPreview}</pre>
      </section>

      <section class="panel result-panel" aria-label="Images">
        <div class="panel-head">
          <h2>Images</h2>
          <button type="button" class="secondary" on:click={loadImages} disabled={loadingImages || generating}>
            Reload
          </button>
        </div>

        {#if selectedImage}
          <div class="selected">
            <img src={imageSource(selectedImage.image_url)} alt="Generated result" />
            <div class="selected-meta">
              <strong>{selectedImage.id}</strong>
              <span>{formatDate(selectedImage.created_at)}</span>
            </div>
          </div>
        {/if}

        <div class="gallery">
          {#each visibleImages as image}
            <button
              type="button"
              class="tile"
              class:selected={selectedImage?.id === image.id}
              on:click={() => selectImage(image)}
            >
              <img src={imageSource(image.image_url)} alt="Generated thumbnail" loading="lazy" />
              <span>{formatDate(image.created_at)}</span>
            </button>
          {/each}
        </div>

        {#if metadata}
          <pre class="metadata">{JSON.stringify(metadata, null, 2)}</pre>
        {/if}
      </section>
    </main>
  </div>
{/if}

<style>
  :global(*) {
    box-sizing: border-box;
  }

  :global(body) {
    margin: 0;
    min-width: 320px;
    background:
      linear-gradient(180deg, rgba(123, 211, 137, 0.08), transparent 280px),
      #11100f;
    color: #f2efe6;
    font-family:
      Inter, ui-sans-serif, system-ui, -apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif;
    letter-spacing: 0;
  }

  button,
  input,
  select,
  textarea {
    font: inherit;
    letter-spacing: 0;
  }

  button {
    border: 1px solid #3a3833;
    border-radius: 8px;
    min-height: 40px;
    padding: 0 14px;
    background: #24231f;
    color: #f2efe6;
    cursor: pointer;
  }

  button:hover:not(:disabled) {
    border-color: #7bd389;
  }

  button:disabled {
    color: #77736c;
    cursor: not-allowed;
  }

  .shell {
    min-height: 100vh;
    padding: 20px;
  }

  .topbar {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 16px;
    max-width: 1820px;
    margin: 0 auto 16px;
  }

  h1,
  h2 {
    margin: 0;
    line-height: 1.1;
  }

  h1 {
    font-size: 28px;
  }

  h2 {
    font-size: 18px;
  }

  .subtle {
    margin-top: 6px;
    color: #aaa49a;
    font-size: 13px;
  }

  .status-line {
    display: flex;
    align-items: center;
    justify-content: flex-end;
    gap: 12px;
    min-width: 240px;
    color: #d6d0c5;
    font-size: 14px;
  }

  .error {
    color: #ff8a8a;
  }

  .notice {
    color: #7bd389;
  }

  .auth-shell {
    display: grid;
    place-items: center;
    min-height: 100vh;
    padding: 20px;
  }

  .auth-card {
    width: min(420px, 100%);
    display: grid;
    gap: 14px;
    padding: 28px;
    border: 1px solid #34332f;
    border-radius: 12px;
    background: rgba(25, 25, 24, 0.96);
    box-shadow: 0 18px 42px rgba(0, 0, 0, 0.28);
  }

  .auth-card form {
    display: grid;
    gap: 12px;
    padding: 0;
  }

  .auth-divider {
    display: flex;
    align-items: center;
    gap: 10px;
    color: #aaa49a;
    font-size: 12px;
  }

  .auth-divider::before,
  .auth-divider::after {
    content: '';
    height: 1px;
    flex: 1;
    background: #34332f;
  }

  .workspace {
    display: grid;
    grid-template-columns: minmax(360px, 0.95fr) minmax(300px, 0.65fr) minmax(360px, 1fr);
    gap: 16px;
    max-width: 1820px;
    margin: 0 auto;
    align-items: start;
  }

  .panel {
    border: 1px solid #34332f;
    border-radius: 8px;
    background: rgba(25, 25, 24, 0.96);
    box-shadow: 0 18px 42px rgba(0, 0, 0, 0.28);
    overflow: hidden;
  }

  .panel-head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
    min-height: 58px;
    padding: 14px 16px;
    border-bottom: 1px solid #34332f;
    background: #1d1c19;
  }

  form {
    display: grid;
    gap: 14px;
    padding: 16px;
  }

  label {
    display: grid;
    gap: 7px;
  }

  label span {
    color: #c4bdb1;
    font-size: 13px;
  }

  input,
  select,
  textarea {
    width: 100%;
    border: 1px solid #34332f;
    border-radius: 8px;
    background: #11100f;
    color: #f2efe6;
    min-height: 40px;
    padding: 9px 10px;
    outline: none;
  }

  textarea {
    resize: vertical;
    line-height: 1.45;
  }

  input:focus,
  select:focus,
  textarea:focus {
    border-color: #63cdda;
    box-shadow: 0 0 0 3px rgba(99, 205, 218, 0.12);
  }

  .segments {
    display: grid;
    grid-template-columns: repeat(3, minmax(0, 1fr));
    width: min(330px, 100%);
    border: 1px solid #34332f;
    border-radius: 8px;
    overflow: hidden;
  }

  .segments button {
    border: 0;
    border-radius: 0;
    min-height: 36px;
    background: #161511;
    color: #aaa49a;
  }

  .segments button.active {
    background: #263629;
    color: #eaffed;
  }

  .two-col,
  .grid-fields,
  .lora-add,
  .lora-head,
  .selected-lora {
    display: grid;
    gap: 12px;
  }

  .two-col {
    grid-template-columns: repeat(2, minmax(0, 1fr));
  }

  .grid-fields {
    grid-template-columns: repeat(3, minmax(0, 1fr));
  }

  h3 {
    margin: 0;
    font-size: 15px;
    line-height: 1.2;
  }

  .lora-box,
  .presets-box {
    display: grid;
    gap: 12px;
    padding: 12px;
    border: 1px solid #34332f;
    border-radius: 8px;
    background: #151411;
  }

  .presets-box {
    margin: 0 16px;
  }

  .presets-head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
  }

  .presets-row {
    display: grid;
    grid-template-columns: minmax(0, 1fr) 76px 76px;
    gap: 10px;
    align-items: center;
  }

  .presets-row.save {
    grid-template-columns: minmax(0, 1fr) 120px;
  }

  .lora-head {
    grid-template-columns: minmax(0, 1fr) minmax(150px, 0.55fr);
    align-items: end;
  }

  .lora-add {
    grid-template-columns: minmax(0, 1fr) 92px 76px;
    align-items: end;
  }

  .add-lora {
    width: 100%;
  }

  .selected-loras {
    display: grid;
    gap: 8px;
  }

  .selected-lora {
    grid-template-columns: minmax(0, 1fr) 92px 92px;
    align-items: center;
    min-height: 58px;
    padding: 8px;
    border: 1px solid #34332f;
    border-radius: 8px;
    background: #11100f;
  }

  .selected-lora div {
    display: grid;
    gap: 4px;
    min-width: 0;
  }

  .selected-lora strong,
  .selected-lora span {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .selected-lora strong {
    color: #f2efe6;
    font-size: 13px;
  }

  .selected-lora span {
    color: #aaa49a;
    font-size: 12px;
  }

  .primary {
    border-color: #7bd389;
    background: #2f6d42;
    color: #ffffff;
    font-weight: 700;
  }

  .secondary {
    background: #171612;
  }

  pre {
    margin: 0;
    padding: 16px;
    max-height: calc(100vh - 122px);
    overflow: auto;
    color: #d9d4ca;
    background: #11100f;
    font-size: 12px;
    line-height: 1.55;
    white-space: pre-wrap;
    word-break: break-word;
  }

  .selected {
    display: grid;
    gap: 10px;
    padding: 16px;
    border-bottom: 1px solid #34332f;
  }

  .selected img {
    width: 100%;
    aspect-ratio: 1 / 1;
    object-fit: contain;
    border-radius: 8px;
    background: #0b0b0a;
  }

  .selected-meta {
    display: grid;
    gap: 5px;
    min-width: 0;
    color: #aaa49a;
    font-size: 13px;
  }

  .selected-meta strong {
    color: #f2efe6;
    overflow-wrap: anywhere;
  }

  .gallery {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(118px, 1fr));
    gap: 10px;
    padding: 16px;
  }

  .tile {
    display: grid;
    gap: 8px;
    height: 166px;
    padding: 8px;
    text-align: left;
    background: #151411;
  }

  .tile.selected {
    border-color: #e8bd68;
  }

  .tile img {
    width: 100%;
    aspect-ratio: 1 / 1;
    object-fit: cover;
    border-radius: 6px;
    background: #0b0b0a;
  }

  .tile span {
    color: #aaa49a;
    font-size: 12px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .metadata {
    border-top: 1px solid #34332f;
    max-height: 360px;
  }

  @media (max-width: 1280px) {
    .workspace {
      grid-template-columns: minmax(340px, 1fr) minmax(340px, 1fr);
    }

    .result-panel {
      grid-column: 1 / -1;
    }
  }

  @media (max-width: 820px) {
    .shell {
      padding: 12px;
    }

    .topbar,
    .workspace {
      display: grid;
      grid-template-columns: 1fr;
    }

    .status-line {
      justify-content: space-between;
      min-width: 0;
    }

    .panel-head,
    .two-col,
    .grid-fields,
    .lora-head,
    .lora-add,
    .selected-lora {
      grid-template-columns: 1fr;
    }

    .panel-head {
      align-items: stretch;
    }

    .segments {
      width: 100%;
    }
  }
</style>
