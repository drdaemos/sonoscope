<script lang="ts">
  import type { Snippet } from "svelte";
  import type { HTMLAttributes } from "svelte/elements";
  import { cn } from "$lib/utils";

  type TabsListProps = HTMLAttributes<HTMLDivElement> & {
    children?: Snippet;
  };

  let { children, class: className, ...restProps }: TabsListProps = $props();
</script>

<div
  class={cn(
    "inline-flex h-9 items-center justify-center rounded-lg bg-muted p-1 text-muted-foreground",
    className,
  )}
  {...restProps}
>
  {@render children?.()}
</div>
