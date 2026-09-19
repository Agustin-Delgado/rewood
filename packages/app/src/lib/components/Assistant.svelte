<script lang="ts">
	/**
	 * Natural language → spec (§43). The server runs the model and the
	 * engine in a loop; what arrives here is a spec that already compiled,
	 * with its findings. Applying it is the same as pasting JSON.
	 */
	import { app } from '$lib/state.svelte';

	type Msg = { role: 'user' | 'assistant'; content: string; note?: string };
	let messages: Msg[] = $state([]);
	let draft = $state('');
	let busy = $state(false);
	let error: string | null = $state(null);

	const SUGGESTIONS = [
		'Un placard de 2 metros con tres puertas y seis cajones',
		'Hacelo 20 cm más ancho',
		'Cambiá las bisagras por las de la biblioteca y sacale los tiradores',
		'Una biblioteca de 80×200×30 con un estante fijo a 1 m'
	];

	async function send(text = draft) {
		const message = text.trim();
		if (!message || busy) return;
		draft = '';
		error = null;
		messages.push({ role: 'user', content: message });
		busy = true;
		try {
			const history = messages.slice(0, -1).map((m) => ({ role: m.role, content: m.content }));
			const r = await app.server.assistant(message, app.spec, history);
			let note: string | undefined;
			if (r.spec) {
				app.applySpec(r.spec);
				const n = r.diagnostics.length;
				note = `spec aplicada · ${r.status}${r.manufacturingBlocked ? ' · BLOQUEADA' : ''}${n ? ` · ${n} hallazgo${n === 1 ? '' : 's'}` : ''}${r.corrections ? ` · ${r.corrections} corrección${r.corrections === 1 ? '' : 'es'} del motor` : ''}`;
			}
			messages.push({ role: 'assistant', content: r.reply || '(sin texto)', note });
		} catch (e) {
			error = (e as Error).message;
			messages.pop();
			draft = message;
		} finally {
			busy = false;
		}
	}
</script>

<div class="assistant">
	{#if app.serverStatus !== 'ok'}
		<p class="muted">Necesita el servidor conectado (pestaña Servidor).</p>
	{:else if !app.assistantModel}
		<p class="muted">
			El servidor no tiene el asistente activo: arrancalo con <code>ANTHROPIC_API_KEY</code> en el entorno.
		</p>
	{:else}
		<p class="muted small">
			Modelo <code>{app.assistantModel}</code>. El modelo escribe la spec; el motor la compila y le devuelve los
			hallazgos hasta que fabrica. Nunca decide una perforación.
		</p>
		<div class="log">
			{#if messages.length === 0}
				<div class="suggestions">
					{#each SUGGESTIONS as s (s)}
						<button onclick={() => send(s)}>{s}</button>
					{/each}
				</div>
			{/if}
			{#each messages as m, i (i)}
				<div class="msg {m.role}">
					<div class="text">{m.content}</div>
					{#if m.note}<div class="note">{m.note}</div>{/if}
				</div>
			{/each}
			{#if busy}<div class="msg assistant"><div class="text muted">pensando y compilando…</div></div>{/if}
			{#if error}<div class="error">{error}</div>{/if}
		</div>
		<form
			class="input"
			onsubmit={(e) => {
				e.preventDefault();
				send();
			}}
		>
			<input bind:value={draft} placeholder="describí el mueble o el cambio…" disabled={busy} />
			<button type="submit" class="primary" disabled={busy || !draft.trim()}>Enviar</button>
		</form>
	{/if}
</div>

<style>
	.assistant {
		display: flex;
		flex-direction: column;
		height: 100%;
		font-size: 12px;
		padding: 8px;
		box-sizing: border-box;
	}
	.log {
		flex: 1;
		overflow: auto;
		display: flex;
		flex-direction: column;
		gap: 6px;
	}
	.msg {
		max-width: 92%;
		padding: 6px 8px;
		border-radius: 8px;
		white-space: pre-wrap;
	}
	.msg.user {
		align-self: flex-end;
		background: #e8f0fb;
	}
	.msg.assistant {
		align-self: flex-start;
		background: #f4f4f4;
	}
	.note {
		margin-top: 4px;
		font-size: 11px;
		color: #2a7;
	}
	.suggestions {
		display: flex;
		flex-direction: column;
		gap: 4px;
	}
	.suggestions button {
		text-align: left;
		font: inherit;
		background: #fff;
		border: 1px dashed #bbb;
		border-radius: 6px;
		padding: 5px 8px;
		cursor: pointer;
		color: #246;
	}
	.input {
		display: flex;
		gap: 6px;
		margin-top: 8px;
	}
	.input input {
		flex: 1;
		font: inherit;
		padding: 5px 8px;
		border: 1px solid #bbb;
		border-radius: 4px;
	}
	button.primary {
		font: inherit;
		background: #ff8c42;
		border: 1px solid #ff8c42;
		color: #fff;
		border-radius: 4px;
		padding: 4px 10px;
		cursor: pointer;
	}
	button:disabled {
		opacity: 0.5;
	}
	.error {
		color: #c33;
		background: #fee;
		padding: 4px 8px;
	}
	.muted {
		color: #666;
	}
	.small {
		margin: 0 0 6px;
	}
	code {
		font-family: ui-monospace, monospace;
	}
</style>
