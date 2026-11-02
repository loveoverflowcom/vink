package vink.ui

import androidx.compose.foundation.Image
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.aspectRatio
import androidx.compose.foundation.layout.defaultMinSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Icon
import androidx.compose.material3.LocalContentColor
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.SolidColor
import androidx.compose.ui.graphics.painter.Painter
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.graphics.vector.addPathNodes
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.clearAndSetSemantics
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.role
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.LinkAnnotation
import androidx.compose.ui.text.SpanStyle
import androidx.compose.ui.text.TextLinkStyles
import androidx.compose.ui.text.buildAnnotatedString
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextDecoration
import androidx.compose.ui.text.withLink
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp

/**
 * Identity and media (shared/components.md). Components receive resolved facts — a name, a
 * stable key, a decoded image — and never look up a profile, an email or an authorised image.
 * Network, authorisation and decoding live in the host's adapter.
 */
enum class IconSize(val dp: Dp) { Small(VinkDimens.SizeIconSm), Medium(VinkDimens.SizeIcon) }

private val vectors = mutableMapOf<IconName, ImageVector>()

/** The registry path as a vector; built once per icon. */
fun IconName.vector(): ImageVector = vectors.getOrPut(this) {
    ImageVector.Builder(id, VinkDimens.SizeIcon, VinkDimens.SizeIcon, VinkIconSource.VIEWPORT, VinkIconSource.VIEWPORT)
        .addPath(addPathNodes(path), fill = SolidColor(Color.Black))
        .build()
}

/** A registry icon tinted with the content colour; without [label] it is decorative. */
@Composable
fun VinkIcon(name: IconName, modifier: Modifier = Modifier, label: String? = null, size: IconSize = IconSize.Medium, tint: Color = LocalContentColor.current) {
    Icon(name.vector(), contentDescription = label, modifier = modifier.size(size.dp), tint = tint)
}

/** A decoded image the host resolved. Only [Ready] shows; a decode failure is the host's [Failed]. */
sealed interface ImageState {
    data object Absent : ImageState

    data object Loading : ImageState

    data class Ready(val painter: Painter) : ImageState

    data object Failed : ImageState
}

/** Wide/narrow sizes; [narrow] is chosen by the host from its window width (< 760dp). */
enum class AvatarSize(val id: String, val wide: Dp, val narrow: Dp) {
    Small("sm", VinkDimens.SizeAvatarSm, VinkDimens.SizeAvatarSm),
    Medium("md", VinkDimens.SizeAvatarMd, VinkDimens.SizeAvatarMd),
    Feed("feed", VinkDimens.SizeAvatarFeed, VinkDimens.SizeAvatarFeedNarrow),
    Spotlight("spotlight", VinkDimens.SizeAvatarSpotlight, VinkDimens.SizeAvatarSpotlightNarrow),
    Reader("reader", VinkDimens.SizeAvatarReader, VinkDimens.SizeAvatarReaderNarrow),
    ;

    fun at(narrow: Boolean): Dp = if (narrow) this.narrow else wide
}

/** Author and account avatars are different identities; one never stands in for the other. */
enum class AvatarKind { Author, Account }

/**
 * A circular avatar: a ready image over the initials, the initials over a person mark. Its tone
 * comes from [identityKey] only. [decorative] hides it when a visible name sits next to it.
 * [kind] marks author vs account for callers and tests; the design amendment defines no visual
 * difference yet, so both render alike.
 */
@Suppress("UNUSED_PARAMETER")
@Composable
fun VinkAvatar(
    name: String?,
    identityKey: String?,
    modifier: Modifier = Modifier,
    image: ImageState = ImageState.Absent,
    size: AvatarSize = AvatarSize.Medium,
    kind: AvatarKind = AvatarKind.Author,
    decorative: Boolean = false,
    narrow: Boolean = false,
) {
    val letters = remember(name) { initials(name) }
    val (bg, fg) = avatarRoles(remember(identityKey) { tone(identityKey) })
    val c = LocalVink.current.colors
    val diameter = size.at(narrow)
    val label = name?.takeIf { it.isNotBlank() } ?: "Chưa có tên hiển thị"
    val semantics = if (decorative) {
        Modifier.clearAndSetSemantics {}
    } else {
        Modifier.clearAndSetSemantics {
            contentDescription = label
            role = Role.Image
        }
    }
    // Pictorial: the circle keeps its token size at any font scale, and so do the initials.
    val letterSize = with(LocalDensity.current) { (diameter * 0.4f).toSp() }
    Box(
        modifier.then(semantics).size(diameter).clip(CircleShape).background(c[bg]),
        contentAlignment = Alignment.Center,
    ) {
        if (letters != null) {
            Text(letters, color = c[fg], fontSize = letterSize, fontWeight = FontWeight.SemiBold, maxLines = 1, softWrap = false)
        } else {
            Icon(IconName.Person.vector(), null, Modifier.size(diameter * 0.6f), tint = c[fg])
        }
        if (image is ImageState.Ready) {
            Image(image.painter, null, Modifier.matchParentSize(), contentScale = ContentScale.Crop)
        }
    }
}

/** Up to [maxVisible] decorative avatars and one group name; no focus stops inside. */
@Composable
fun VinkAvatarGroup(
    people: List<Person>,
    label: String,
    modifier: Modifier = Modifier,
    maxVisible: Int = 3,
    total: Int? = null,
    size: AvatarSize = AvatarSize.Small,
) {
    val c = LocalVink.current.colors
    val summary = groupSummary(label, people, maxVisible, total)
    val visible = people.take(maxVisible)
    val more = overflow(visible.size, total)
    val diameter = size.at(false)
    Row(
        modifier.clearAndSetSemantics { contentDescription = summary },
        horizontalArrangement = Arrangement.spacedBy(-VinkDimens.Space2),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        visible.forEach { p ->
            VinkAvatar(p.name, p.key, Modifier.border(2.dp, c[ColorRole.Surface], CircleShape), image = p.image, size = size, decorative = true)
        }
        if (more != null) {
            Box(
                Modifier.defaultMinSize(diameter, diameter).border(2.dp, c[ColorRole.Surface], CircleShape)
                    .background(c[ColorRole.SurfaceContainerHighest], CircleShape).padding(horizontal = VinkDimens.Space1),
                contentAlignment = Alignment.Center,
            ) {
                Text("+$more", style = LocalVink.current.text(VinkTextRoles.Caption), color = c[ColorRole.OnSurface], fontWeight = FontWeight.SemiBold)
            }
        }
    }
}

/**
 * Author name (bold; a link only when the host passes [onOpenProfile] for a verified profile)
 * before muted metadata. The avatar is decorative so the name is read once.
 */
@Composable
fun VinkAuthorByline(
    name: String,
    identityKey: String?,
    modifier: Modifier = Modifier,
    image: ImageState = ImageState.Absent,
    meta: List<String> = emptyList(),
    size: AvatarSize = AvatarSize.Feed,
    narrow: Boolean = false,
    onOpenProfile: (() -> Unit)? = null,
) {
    val vink = LocalVink.current
    val blank = name.isBlank()
    val shown = if (blank) "Chưa có tên tác giả" else name
    Row(modifier, horizontalArrangement = Arrangement.spacedBy(VinkDimens.Space3), verticalAlignment = Alignment.CenterVertically) {
        VinkAvatar(name.takeUnless { blank }, identityKey, image = image, size = size, decorative = true, narrow = narrow)
        Column(Modifier.weight(1f, fill = false)) {
            if (onOpenProfile != null && !blank) {
                // Same look as the Web byline link: the bold name colour, underlined.
                val link = SpanStyle(color = vink.colors[ColorRole.OnSurface], fontWeight = FontWeight.SemiBold, textDecoration = TextDecoration.Underline)
                Text(
                    buildAnnotatedString { withLink(LinkAnnotation.Clickable("profile", TextLinkStyles(link)) { onOpenProfile() }) { append(shown) } },
                    style = vink.text(VinkTextRoles.Body),
                )
            } else {
                Text(
                    shown,
                    style = vink.text(VinkTextRoles.Body),
                    fontWeight = if (blank) null else FontWeight.SemiBold,
                    color = vink.colors[if (blank) ColorRole.OnSurfaceVariant else ColorRole.OnSurface],
                )
            }
            val details = meta.filter { it.isNotBlank() }
            if (details.isNotEmpty()) VinkText(details.joinToString(" · "), role = TypeRole.Caption, tone = TextTone.Muted)
        }
    }
}

enum class AspectRatio(val id: String, val ratio: Float) { Square("square", 1f), Landscape("landscape", 4f / 3f), Wide("wide", 16f / 9f) }

/** A fixed-ratio image; absent, loading and failed images show a quiet neutral fallback with text. */
@Composable
fun VinkImageFrame(image: ImageState, alt: String, modifier: Modifier = Modifier, ratio: AspectRatio = AspectRatio.Landscape) {
    val c = LocalVink.current.colors
    val decorative = alt.isBlank()
    Box(
        modifier.fillMaxWidth().aspectRatio(ratio.ratio).clip(RoundedCornerShape(VinkDimens.RadiusLg)).background(c[ColorRole.SurfaceContainerHigh]),
        contentAlignment = Alignment.Center,
    ) {
        if (image is ImageState.Ready) {
            Image(image.painter, alt.takeUnless { decorative }, Modifier.matchParentSize(), contentScale = ContentScale.Crop)
        } else {
            val text = when (image) {
                ImageState.Failed -> "Không tải được ảnh"
                ImageState.Loading -> "Đang tải ảnh"
                else -> "Chưa có ảnh"
            }
            Column(
                Modifier.clearAndSetSemantics { if (!decorative) contentDescription = "$alt — $text" }.padding(VinkDimens.Space3),
                horizontalAlignment = Alignment.CenterHorizontally,
                verticalArrangement = Arrangement.spacedBy(VinkDimens.Space2),
            ) {
                VinkIcon(IconName.Image, tint = c[ColorRole.OnSurfaceVariant])
                VinkText(text, role = TypeRole.Caption, tone = TextTone.Muted)
            }
        }
    }
}
