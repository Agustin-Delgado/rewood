<script lang="ts" module>
	export type SelectOption<T extends string = string> = {
		value: T;
		label: string;
		/** Secondary text at the right of the option (a size, a code). */
		hint?: string;
		disabled?: boolean;
		/** Consecutive options with the same group are listed under that heading. */
		group?: string;
	};
</script>

<script lang="ts" generics="V extends string">
	import { ListBox } from '@human-kit/ui/listbox';
	import { Popover } from '@human-kit/ui/popover';
	import Check from '@lucide/svelte/icons/check';
	import ChevronsUpDown from '@lucide/svelte/icons/chevrons-up-down';
	import { floating, select } from './recipes';

	type Props = {
		options: SelectOption<V>[];
		value: V | null | undefined;
		onChange?: (value: V) => void;
		placeholder?: string;
		size?: 'xs' | 'sm' | 'md';
		disabled?: boolean;
		invalid?: boolean;
		id?: string;
		class?: string;
		title?: string;
		'aria-label'?: string;
	};

	let {
		options,
		value = $bindable(),
		onChange,
		placeholder = 'Elegir…',
		size,
		disabled = false,
		invalid = false,
		id,
		class: className,
		title,
		'aria-label': ariaLabel
	}: Props = $props();

	let open = $state(false);
	let trigger: HTMLElement | null = $state(null);
	const r = $derived(select({ size }));
	const f = floating();
	const current = $derived(options.find((o) => o.value === value));
	const groups = $derived.by(() => {
		const out: { name: string | undefined; items: SelectOption<V>[] }[] = [];
		for (const o of options) {
			const last = out[out.length - 1];
			if (last && last.name === o.group) last.items.push(o);
			else out.push({ name: o.group, items: [o] });
		}
		return out;
	});

	function pick(next: Set<string | number>) {
		const v = [...next][0] as V | undefined;
		open = false;
		if (v === undefined || v === value) return;
		value = v;
		onChange?.(v);
	}
</script>

<Popover.Root bind:open bind:triggerRef={trigger}>
	<Popover.Trigger
		{id}
		{disabled}
		{title}
		aria-label={ariaLabel}
		data-invalid={invalid || undefined}
		class={r.trigger({ class: className })}
	>
		{#if current}
			<span class={r.value()}>{current.label}</span>
		{:else}
			<!-- A value the options do not list is still shown, so nothing is hidden. -->
			<span class={r.placeholder()}>{value || placeholder}</span>
		{/if}
		<ChevronsUpDown />
	</Popover.Trigger>
	<Popover.Content
		placement="bottom-start"
		offset={4}
		class={f.content()}
	>
		<!-- Sized here: the content's own style attribute carries its position. -->
		<div style="width: max({(trigger?.offsetWidth ?? 0) - 10}px, 11rem); max-width: 28rem">
		<ListBox.Root
			value={value == null ? [] : [value]}
			onChange={pick}
			selectionMode="single"
			aria-label={ariaLabel ?? placeholder}
			class={f.list()}
		>
			{#each groups as g, i (i)}
				{#if g.name}<div class={f.label()} role="presentation">{g.name}</div>{/if}
				{#each g.items as o (o.value)}
					<ListBox.Item id={o.value} textValue={o.label} disabled={o.disabled} class={f.item({ class: 'pl-6' })}>
						{#if o.value === value}<Check class="absolute left-1.5" />{/if}
						<span class="min-w-0 truncate">{o.label}</span>
						{#if o.hint}<span class={f.itemHint()}>{o.hint}</span>{/if}
					</ListBox.Item>
				{/each}
			{/each}
		</ListBox.Root>
		</div>
	</Popover.Content>
</Popover.Root>
