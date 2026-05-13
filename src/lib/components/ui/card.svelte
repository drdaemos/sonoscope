<script lang="ts">
  import type { Snippet } from "svelte";
  import type { HTMLAttributes } from "svelte/elements";
  import { cn } from "$lib/utils";

  type CardProps = HTMLAttributes<HTMLDivElement> & {
    children?: Snippet;
  };

  let { children, class: className, ...restProps }: CardProps = $props();
</script>

<div
  class={cn("rounded-lg border bg-card text-card-foreground shadow-xs", className)}
  {...restProps}
>
  {@render children?.()}
</div>
