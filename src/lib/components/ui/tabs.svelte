<script lang="ts">
  import type { Snippet } from "svelte";
  import type { HTMLAttributes } from "svelte/elements";
  import { cn } from "$lib/utils";

  type TabsProps = HTMLAttributes<HTMLDivElement> & {
    children?: Snippet;
  };

  let { children, class: className, ...restProps }: TabsProps = $props();
</script>

<div class={cn("flex items-center", className)} {...restProps}>
  {@render children?.()}
</div>
