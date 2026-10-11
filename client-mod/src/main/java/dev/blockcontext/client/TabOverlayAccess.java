package dev.blockcontext.client;
import net.minecraft.client.gui.components.PlayerTabOverlay;
import net.minecraft.network.chat.Component;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.gen.Accessor;
@Mixin(PlayerTabOverlay.class)
public interface TabOverlayAccess {
    @Accessor("header") Component blockcontext$header();
    @Accessor("footer") Component blockcontext$footer();
}
