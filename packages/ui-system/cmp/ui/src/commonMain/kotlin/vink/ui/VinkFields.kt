package vink.ui

import androidx.compose.foundation.border
import androidx.compose.foundation.interaction.MutableInteractionSource
import androidx.compose.foundation.interaction.collectIsFocusedAsState
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.selection.selectable
import androidx.compose.foundation.selection.selectableGroup
import androidx.compose.foundation.selection.toggleable
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.KeyboardActions
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material3.Checkbox
import androidx.compose.material3.CheckboxDefaults
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.ExposedDropdownMenuBox
import androidx.compose.material3.ExposedDropdownMenuDefaults
import androidx.compose.material3.MenuAnchorType
import androidx.compose.material3.TextField
import androidx.compose.material3.TextFieldDefaults
import androidx.compose.material3.TextFieldColors
import androidx.compose.material3.RadioButton
import androidx.compose.material3.RadioButtonDefaults
import androidx.compose.material3.Switch
import androidx.compose.material3.SwitchDefaults
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.semantics.LiveRegionMode
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.clearAndSetSemantics
import androidx.compose.ui.semantics.error
import androidx.compose.ui.semantics.liveRegion
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.semantics.text
import androidx.compose.ui.text.AnnotatedString
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.unit.dp
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.text.input.TextFieldValue

/**
 * Inputs and choices (shared/components.md). Text fields take and return [TextFieldValue] so
 * selection and IME composition survive recomposition — never a String round trip. Values are
 * passed back raw (no trim/normalise/parse). Floating labels use Material's `label` slot. External labels are visible adjacent text,
 * named in the native field semantics (Text, not contentDescription). Both remain present
 * independently of the placeholder. Help and error are adjacent text and
 * `error()` semantics. Validation is the caller's.
 */
enum class FieldMode { Editable, ReadOnly, Disabled }

enum class FieldVariant { ExternalLabel, FloatingTonal }

enum class TextKind(val keyboard: KeyboardType) { Text(KeyboardType.Text), Email(KeyboardType.Email), Url(KeyboardType.Uri), Tel(KeyboardType.Phone) }

data class ChoiceOption(val value: String, val label: String, val disabled: Boolean = false)

private fun String?.present(): String? = this?.takeIf { it.isNotBlank() }

private fun accessibleName(label: String, required: Boolean) = if (required) "$label (bắt buộc)" else label


@Composable
private fun Messages(help: String?, error: String?) {
    if (help != null) VinkText(help, role = TypeRole.Caption, tone = TextTone.Muted)
    if (error != null) {
        Row(horizontalArrangement = Arrangement.spacedBy(VinkDimens.Space1), verticalAlignment = Alignment.Top) {
            VinkIcon(IconName.Error, size = IconSize.Small, tint = LocalVink.current.colors[ColorRole.Error])
            VinkText(error, role = TypeRole.Label, tone = TextTone.Error)
        }
    }
}

/** The native field retains selection/composition. Only its paint changes. */
@Composable
private fun tonalColors(): TextFieldColors {
    val c = LocalVink.current.colors
    return TextFieldDefaults.colors(
        focusedTextColor = c[ColorRole.OnSurface],
        unfocusedTextColor = c[ColorRole.OnSurface],
        errorTextColor = c[ColorRole.OnSurface],
        focusedContainerColor = c[ColorRole.FieldContainer],
        unfocusedContainerColor = c[ColorRole.FieldContainer],
        errorContainerColor = c[ColorRole.FieldContainer],
        disabledContainerColor = c[ColorRole.DisabledContainer],
        disabledTextColor = c[ColorRole.DisabledContent],
        focusedIndicatorColor = Color.Transparent,
        unfocusedIndicatorColor = Color.Transparent,
        errorIndicatorColor = Color.Transparent,
        disabledIndicatorColor = Color.Transparent,
        cursorColor = c[ColorRole.Primary],
        errorCursorColor = c[ColorRole.Error],
        focusedLabelColor = c[ColorRole.OnSurfaceVariant],
        unfocusedLabelColor = c[ColorRole.OnSurfaceVariant],
        errorLabelColor = c[ColorRole.Error],
        disabledLabelColor = c[ColorRole.DisabledContent],
    )
}

@Composable
private fun Modifier.tonalPerimeter(focused: Boolean, invalid: Boolean, enabled: Boolean): Modifier {
    if (!enabled || (!focused && !invalid)) return this
    val c = LocalVink.current.colors
    return border(if (focused) 2.dp else 1.dp, c[if (invalid) ColorRole.Error else ColorRole.FocusRing], RoundedCornerShape(VinkDimens.RadiusMd))
}

private enum class TextControl { Field, Area, Search }

@Composable
private fun TextControlField(
    control: TextControl,
    label: String,
    value: TextFieldValue,
    onValueChange: (TextFieldValue) -> Unit,
    modifier: Modifier,
    help: String?,
    error: String?,
    required: Boolean,
    mode: FieldMode,
    kind: TextKind,
    placeholder: String?,
    minLines: Int,
    onSubmit: ((String) -> Unit)?,
    variant: FieldVariant,
) {
    val vink = LocalVink.current
    val c = vink.colors
    val shown = error.present()
    val name = accessibleName(label, required)
    val interaction = remember { MutableInteractionSource() }
    val focused by interaction.collectIsFocusedAsState()
    val floating = variant == FieldVariant.FloatingTonal && control != TextControl.Area
    val description = help ?: if (mode == FieldMode.Disabled) "Tính năng chưa khả dụng." else null
    Column(modifier, verticalArrangement = Arrangement.spacedBy(VinkDimens.Space1)) {
        if (!floating) Text(name, Modifier.clearAndSetSemantics {}, style = vink.text(VinkTextRoles.Label), color = c[ColorRole.OnSurface])
        TextField(
            value = value,
            onValueChange = onValueChange,
            modifier = Modifier.fillMaxWidth().heightIn(min = maxOf(48.dp, VinkDimens.DensityControlHeight))
                .tonalPerimeter(focused, shown != null, mode != FieldMode.Disabled)
                .semantics {
                    if (!floating) text = AnnotatedString(name)
                    if (shown != null) error(shown)
                },
            interactionSource = interaction,
            label = if (floating) { { Text(name, style = vink.text(VinkTextRoles.Label)) } } else null,
            enabled = mode != FieldMode.Disabled,
            readOnly = mode == FieldMode.ReadOnly,
            textStyle = vink.text(VinkTextRoles.Body),
            placeholder = placeholder?.let { { Text(it, style = vink.text(VinkTextRoles.Body)) } },
            leadingIcon = if (control == TextControl.Search) {
                { VinkIcon(IconName.Search) }
            } else {
                null
            },
            trailingIcon = if (control == TextControl.Search && value.text.isNotEmpty() && mode == FieldMode.Editable) {
                { VinkIconButton(IconName.Close, "Xóa chữ tìm", onPress = { onValueChange(TextFieldValue("")) }) }
            } else {
                null
            },
            isError = shown != null,
            keyboardOptions = KeyboardOptions(
                keyboardType = kind.keyboard,
                imeAction = when (control) {
                    TextControl.Search -> ImeAction.Search
                    TextControl.Area -> ImeAction.Default
                    TextControl.Field -> ImeAction.Next
                },
            ),
            keyboardActions = KeyboardActions(onSearch = { if (value.composition == null) onSubmit?.invoke(value.text) }),
            singleLine = control != TextControl.Area,
            minLines = minLines,
            shape = RoundedCornerShape(VinkDimens.RadiusMd),
            colors = tonalColors(),
        )
        Messages(description, shown)
    }
}

@Composable
fun VinkTextField(
    label: String,
    value: TextFieldValue,
    onValueChange: (TextFieldValue) -> Unit,
    modifier: Modifier = Modifier,
    help: String? = null,
    error: String? = null,
    required: Boolean = false,
    mode: FieldMode = FieldMode.Editable,
    kind: TextKind = TextKind.Text,
    placeholder: String? = null,
    variant: FieldVariant = FieldVariant.ExternalLabel,
) = TextControlField(TextControl.Field, label, value, onValueChange, modifier, help, error, required, mode, kind, placeholder, 1, null, variant)

/** Multi-line text; newlines, blank lines and leading spaces are kept verbatim. */
@Composable
fun VinkTextArea(
    label: String,
    value: TextFieldValue,
    onValueChange: (TextFieldValue) -> Unit,
    modifier: Modifier = Modifier,
    help: String? = null,
    error: String? = null,
    required: Boolean = false,
    mode: FieldMode = FieldMode.Editable,
    rows: Int = 4,
    placeholder: String? = null,
) = TextControlField(TextControl.Area, label, value, onValueChange, modifier, help, error, required, mode, TextKind.Text, placeholder, rows, null, FieldVariant.ExternalLabel)

/** Search input with a clear button while it has text; the IME Search action calls [onSubmit]. */
@Composable
fun VinkSearchField(
    label: String,
    value: TextFieldValue,
    onValueChange: (TextFieldValue) -> Unit,
    modifier: Modifier = Modifier,
    onSubmit: ((String) -> Unit)? = null,
    help: String? = null,
    mode: FieldMode = FieldMode.Editable,
    placeholder: String? = null,
) = TextControlField(TextControl.Search, label, value, onValueChange, modifier, help, null, false, mode, TextKind.Text, placeholder, 1, onSubmit, FieldVariant.FloatingTonal)

@Composable
private fun Toggle(
    switch: Boolean,
    label: String,
    checked: Boolean,
    onChange: (Boolean) -> Unit,
    modifier: Modifier,
    help: String?,
    error: String?,
    enabled: Boolean,
) {
    val c = LocalVink.current.colors
    val shown = error.present()
    Column(modifier, verticalArrangement = Arrangement.spacedBy(VinkDimens.Space1)) {
        // Box and label are one toggleable target with one handler.
        Row(
            Modifier.heightIn(min = maxOf(48.dp, VinkDimens.DensityControlHeight))
                .toggleable(checked, enabled = enabled, role = if (switch) Role.Switch else Role.Checkbox, onValueChange = onChange)
                .semantics(mergeDescendants = true) { if (shown != null) error(shown) },
            horizontalArrangement = Arrangement.spacedBy(VinkDimens.Space3),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            if (switch) {
                Switch(
                    checked,
                    onCheckedChange = null,
                    enabled = enabled,
                    colors = SwitchDefaults.colors(checkedTrackColor = c[ColorRole.Primary], checkedThumbColor = c[ColorRole.OnPrimary]),
                )
            } else {
                Checkbox(
                    checked,
                    onCheckedChange = null,
                    enabled = enabled,
                    colors = CheckboxDefaults.colors(
                        checkedColor = c[ColorRole.Primary],
                        uncheckedColor = if (shown != null) c[ColorRole.Error] else c[ColorRole.Outline],
                    ),
                )
            }
            VinkText(label, tone = if (enabled) TextTone.Default else TextTone.Muted)
            if (switch) {
                VinkText(if (checked) "Bật" else "Tắt", Modifier.clearAndSetSemantics {}, TypeRole.Caption, TextTone.Muted)
            }
        }
        Messages(help, shown)
    }
}

@Composable
fun VinkCheckbox(
    label: String,
    checked: Boolean,
    onChange: (Boolean) -> Unit,
    modifier: Modifier = Modifier,
    help: String? = null,
    error: String? = null,
    enabled: Boolean = true,
) = Toggle(false, label, checked, onChange, modifier, help, error, enabled)

/** An on/off switch (`Role.Switch`) with a visible Bật/Tắt state. */
@Composable
fun VinkSwitch(
    label: String,
    checked: Boolean,
    onChange: (Boolean) -> Unit,
    modifier: Modifier = Modifier,
    help: String? = null,
    enabled: Boolean = true,
) = Toggle(true, label, checked, onChange, modifier, help, null, enabled)

/** One of several options in a selectable group; each row is one target. */
@Composable
fun VinkRadioGroup(
    legend: String,
    options: List<ChoiceOption>,
    value: String?,
    onChange: (String) -> Unit,
    modifier: Modifier = Modifier,
    help: String? = null,
    error: String? = null,
    required: Boolean = false,
    enabled: Boolean = true,
) {
    val c = LocalVink.current.colors
    val shown = error.present()
    Column(modifier, verticalArrangement = Arrangement.spacedBy(VinkDimens.Space1)) {
        Row(Modifier.semantics(mergeDescendants = true) {}) {
            VinkText(legend, role = TypeRole.Label)
            if (required) VinkText(" (bắt buộc)", role = TypeRole.Caption, tone = TextTone.Muted)
        }
        Column(Modifier.selectableGroup().semantics { if (shown != null) error(shown) }) {
            options.forEach { option ->
                val on = enabled && !option.disabled
                Row(
                    Modifier.fillMaxWidth().heightIn(min = maxOf(48.dp, VinkDimens.DensityControlHeight))
                        .selectable(option.value == value, enabled = on, role = Role.RadioButton) { onChange(option.value) },
                    horizontalArrangement = Arrangement.spacedBy(VinkDimens.Space3),
                    verticalAlignment = Alignment.CenterVertically,
                ) {
                    RadioButton(
                        option.value == value,
                        onClick = null,
                        enabled = on,
                        colors = RadioButtonDefaults.colors(
                            selectedColor = c[ColorRole.Primary],
                            unselectedColor = if (shown != null) c[ColorRole.Error] else c[ColorRole.Outline],
                        ),
                    )
                    VinkText(option.label, tone = if (on) TextTone.Default else TextTone.Muted)
                }
            }
        }
        Messages(help, shown)
    }
}

/**
 * Material's exposed dropdown (the platform-native menu). Without [required], the first item
 * clears the value (`null`); with it, the placeholder only shows until a value is chosen.
 */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun VinkSelect(
    label: String,
    options: List<ChoiceOption>,
    value: String?,
    onChange: (String?) -> Unit,
    modifier: Modifier = Modifier,
    placeholder: String = "Chọn…",
    help: String? = null,
    error: String? = null,
    required: Boolean = false,
    enabled: Boolean = true,
) {
    val vink = LocalVink.current
    val c = vink.colors
    val shown = error.present()
    var expanded by remember { mutableStateOf(false) }
    val interaction = remember { MutableInteractionSource() }
    val focused by interaction.collectIsFocusedAsState()
    val current = options.firstOrNull { it.value == value }?.label
    Column(modifier, verticalArrangement = Arrangement.spacedBy(VinkDimens.Space1)) {
        ExposedDropdownMenuBox(expanded = expanded, onExpandedChange = { if (enabled) expanded = it }) {
            TextField(
                value = current ?: placeholder,
                onValueChange = {},
                readOnly = true,
                enabled = enabled,
                textStyle = vink.text(VinkTextRoles.Body).copy(color = if (current == null) c[ColorRole.OnSurfaceVariant] else c[ColorRole.OnSurface]),
                trailingIcon = { ExposedDropdownMenuDefaults.TrailingIcon(expanded) },
                isError = shown != null,
                singleLine = true,
                shape = RoundedCornerShape(VinkDimens.RadiusMd),
                colors = tonalColors(),
                interactionSource = interaction,
                label = { Text(accessibleName(label, required)) },
                modifier = Modifier.fillMaxWidth().heightIn(min = maxOf(48.dp, VinkDimens.DensityControlHeight))
                    .tonalPerimeter(focused, shown != null, enabled)
                    .menuAnchor(MenuAnchorType.PrimaryNotEditable, enabled)
                    .semantics { if (shown != null) error(shown) },
            )
            ExposedDropdownMenu(expanded = expanded, onDismissRequest = { expanded = false }) {
                if (!required) {
                    DropdownMenuItem(text = { Text(placeholder) }, onClick = {
                        onChange(null)
                        expanded = false
                    })
                }
                options.forEach { option ->
                    DropdownMenuItem(
                        text = { Text(option.label) },
                        enabled = !option.disabled,
                        onClick = {
                            onChange(option.value)
                            expanded = false
                        },
                    )
                }
            }
        }
        Messages(help, shown)
    }
}

/**
 * A caller-built error summary, announced politely-assertively when it appears; each item calls
 * [onSelect] with its field key so the host can move focus (FocusRequester) to that field.
 */
@Composable
fun VinkFormSummary(title: String, items: List<Pair<String, String>>, onSelect: (String) -> Unit, modifier: Modifier = Modifier) {
    if (items.isEmpty()) return
    val c = LocalVink.current.colors
    Column(
        modifier.fillMaxWidth().background(c[ColorRole.ErrorContainer], RoundedCornerShape(VinkDimens.RadiusMd))
            .semantics { liveRegion = LiveRegionMode.Assertive }.padding(VinkDimens.Space4),
        verticalArrangement = Arrangement.spacedBy(VinkDimens.Space2),
    ) {
        Row(horizontalArrangement = Arrangement.spacedBy(VinkDimens.Space2), verticalAlignment = Alignment.CenterVertically) {
            VinkIcon(IconName.Error, size = IconSize.Small, tint = c[ColorRole.OnErrorContainer])
            Text(title, style = LocalVink.current.text(VinkTextRoles.Label), color = c[ColorRole.OnErrorContainer])
        }
        items.forEach { (field, message) ->
            Text(
                "• $message",
                Modifier.heightIn(min = VinkDimens.SizeHitTarget).clickable(role = Role.Button) { onSelect(field) },
                style = LocalVink.current.text(VinkTextRoles.Body),
                color = c[ColorRole.OnErrorContainer],
                textDecoration = androidx.compose.ui.text.style.TextDecoration.Underline,
            )
        }
    }
}
