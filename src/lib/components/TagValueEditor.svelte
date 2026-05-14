<script lang="ts">
  import { Select as SelectPrimitive } from "bits-ui";
  import { Button, Input, Select, SelectContent, SelectItem, SelectTrigger } from "$lib/components/ui";
  import type { TagDimension } from "$lib/stores/library";

  type Props = {
    dimension: TagDimension;
    value: string;
    label: string;
    onValueChange: (value: string) => void;
    onSave: () => void | Promise<void>;
    onClear: () => void | Promise<void>;
    onCancel: () => void;
  };

  let { dimension, value, label, onValueChange, onSave, onClear, onCancel }: Props = $props();

  let canSave = $derived(value.trim().length > 0);
  let isOptionDimension = $derived(["enum", "multi_enum"].includes(dimension.value_type));
</script>

<div class="flex flex-wrap items-center gap-2">
  <span class="text-sm font-medium">{label}</span>

  {#if isOptionDimension}
    <Select
      type="single"
      value={value}
      onValueChange={(nextValue) => {
        if (nextValue) onValueChange(nextValue);
      }}
      disabled={dimension.values.length === 0}
    >
      <SelectTrigger size="sm" class="w-40">
        <SelectPrimitive.Value placeholder="Select value..." />
      </SelectTrigger>
      <SelectContent>
        {#each dimension.values as option}
          <SelectItem value={option}>{option}</SelectItem>
        {/each}
      </SelectContent>
    </Select>
  {:else if dimension.value_type === "numeric"}
    <Input
      type="number"
      class="w-32"
      value={value}
      oninput={(event) => onValueChange(event.currentTarget.value)}
    />
  {:else}
    <Input
      class="w-48"
      value={value}
      oninput={(event) => onValueChange(event.currentTarget.value)}
    />
  {/if}

  <Button size="sm" onclick={onSave} disabled={!canSave}>Save</Button>
  <Button variant="outline" size="sm" onclick={onClear}>Clear user tag</Button>
  <Button variant="ghost" size="sm" onclick={onCancel}>Cancel</Button>
</div>
