import { render, screen, fireEvent } from "@testing-library/svelte";
import { describe, expect, it, vi } from "vitest";
import TagValueEditor from "./TagValueEditor.svelte";

describe("TagValueEditor", () => {
  it("edits numeric dimension values", async () => {
    const onValueChange = vi.fn();
    const onSave = vi.fn();
    const onClear = vi.fn();
    const onCancel = vi.fn();

    render(TagValueEditor, {
      props: {
        dimension: { name: "Tempo", value_type: "numeric", values: [] },
        value: "128",
        label: "Edit Tempo",
        onValueChange,
        onSave,
        onClear,
        onCancel,
      },
    });

    expect(screen.getByText("Edit Tempo")).toBeInTheDocument();
    await fireEvent.input(screen.getByRole("spinbutton"), { target: { value: "130" } });
    expect(onValueChange).toHaveBeenCalledWith("130");

    await fireEvent.click(screen.getByRole("button", { name: "Save" }));
    expect(onSave).toHaveBeenCalledTimes(1);
  });

  it("disables save for empty values", () => {
    render(TagValueEditor, {
      props: {
        dimension: { name: "Tempo", value_type: "numeric", values: [] },
        value: "",
        label: "Edit Tempo",
        onValueChange: vi.fn(),
        onSave: vi.fn(),
        onClear: vi.fn(),
        onCancel: vi.fn(),
      },
    });

    expect(screen.getByRole("button", { name: "Save" })).toBeDisabled();
  });
});
