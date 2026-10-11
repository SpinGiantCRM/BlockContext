package dev.blockcontext.test;

import net.fabricmc.api.EnvType;
import net.fabricmc.loader.impl.launch.knot.Knot;

/** Real Fabric transformation and collector initialization, without launching the game. */
public final class FabricStartupTest {
    public static void main(String[] args) throws Exception {
        Knot knot = new Knot(EnvType.CLIENT);
        ClassLoader target = knot.init(new String[]{"--version", "26.2", "--gameDir", args[0]});
        Class<?> entry = target.loadClass("dev.blockcontext.client.BlockContextClient");
        Object collector = entry.getConstructor().newInstance();
        entry.getMethod("onInitializeClient").invoke(collector);
        Class<?> overlay = target.loadClass("net.minecraft.client.gui.components.PlayerTabOverlay");
        Class<?> access = target.loadClass("dev.blockcontext.client.mixin.TabOverlayAccess");
        if (!access.isAssignableFrom(overlay)) throw new AssertionError("Tab accessors were not applied");
        for (String name : new String[]{"blockcontext$header", "blockcontext$footer"}) {
            if (!overlay.getMethod(name).getReturnType().getName().equals("net.minecraft.network.chat.Component"))
                throw new AssertionError("Incorrect accessor return type");
        }
        if (java.nio.file.Files.exists(java.nio.file.Path.of(args[0], "blockcontext/latest.json")))
            throw new AssertionError("Initialization must not create player observations");
        System.out.println("FabricStartup: collector initialized and both tab accessors transformed successfully; game not launched");
    }
}
