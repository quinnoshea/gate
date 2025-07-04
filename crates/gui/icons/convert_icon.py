#!/usr/bin/env python3
import subprocess
import sys
import os

# Try to import cairosvg, if not available fall back to other methods
try:
    import cairosvg
    has_cairosvg = True
except ImportError:
    has_cairosvg = False

def convert_svg_to_png(svg_path, png_path, width, height):
    if has_cairosvg:
        cairosvg.svg2png(url=svg_path, write_to=png_path, 
                         output_width=width, output_height=height)
        print(f"Created {png_path} ({width}x{height}) using cairosvg")
    else:
        # Try using Pillow with cairosvg as a fallback
        try:
            from PIL import Image
            import io
            
            # Read SVG and convert using subprocess with rsvg-convert if available
            if subprocess.run(['which', 'rsvg-convert'], capture_output=True).returncode == 0:
                subprocess.run([
                    'rsvg-convert', '-w', str(width), '-h', str(height),
                    svg_path, '-o', png_path
                ])
                print(f"Created {png_path} ({width}x{height}) using rsvg-convert")
            else:
                print(f"Error: No suitable SVG converter found for {png_path}")
                return False
        except Exception as e:
            print(f"Error converting {svg_path}: {e}")
            return False
    return True

def create_ico_file(png_sizes, ico_path):
    """Create Windows .ico file from multiple PNG sizes"""
    try:
        from PIL import Image
        
        # Load all PNG images
        images = []
        for size, path in png_sizes:
            if os.path.exists(path):
                img = Image.open(path)
                images.append(img)
        
        if images:
            # Save as ICO with multiple sizes
            images[0].save(ico_path, format='ICO', sizes=[(img.width, img.height) for img in images])
            print(f"Created {ico_path} with {len(images)} sizes")
            return True
    except ImportError:
        print("PIL/Pillow not available, skipping .ico generation")
    except Exception as e:
        print(f"Error creating .ico: {e}")
    return False

def create_icns_file(png_path, icns_path):
    """Create macOS .icns file"""
    # Try using iconutil (macOS native tool)
    if subprocess.run(['which', 'iconutil'], capture_output=True).returncode == 0:
        # Need to create iconset directory structure
        iconset_path = icns_path.replace('.icns', '.iconset')
        os.makedirs(iconset_path, exist_ok=True)
        
        # Define required sizes for icns
        sizes = [
            (16, 'icon_16x16.png'),
            (32, 'icon_16x16@2x.png'),
            (32, 'icon_32x32.png'),
            (64, 'icon_32x32@2x.png'),
            (128, 'icon_128x128.png'),
            (256, 'icon_128x128@2x.png'),
            (256, 'icon_256x256.png'),
            (512, 'icon_256x256@2x.png'),
            (512, 'icon_512x512.png'),
            (1024, 'icon_512x512@2x.png'),
        ]
        
        # Convert to required sizes
        for size, filename in sizes:
            output_path = os.path.join(iconset_path, filename)
            convert_svg_to_png('gate-icon.svg', output_path, size, size)
        
        # Convert iconset to icns
        subprocess.run(['iconutil', '-c', 'icns', iconset_path, '-o', icns_path])
        
        # Clean up iconset
        subprocess.run(['rm', '-rf', iconset_path])
        print(f"Created {icns_path} using iconutil")
        return True
    else:
        print("iconutil not available (not on macOS), skipping .icns generation")
        return False

# Main conversion
if __name__ == "__main__":
    # Change to icons directory
    os.chdir(os.path.dirname(os.path.abspath(__file__)))
    
    # Convert to required PNG sizes
    sizes = [
        (32, '32x32.png'),
        (128, '128x128.png'),
        (256, '128x128@2x.png'),
    ]
    
    png_files = []
    for size, filename in sizes:
        if convert_svg_to_png('gate-icon.svg', filename, size, size):
            png_files.append((size, filename))
    
    # Create .ico file for Windows
    if png_files:
        # Add more sizes for better .ico
        extra_sizes = [(16, '16x16.png'), (48, '48x48.png'), (256, '256x256.png')]
        for size, filename in extra_sizes:
            if not os.path.exists(filename):
                convert_svg_to_png('gate-icon.svg', filename, size, size)
                png_files.append((size, filename))
        
        create_ico_file(png_files, 'icon.ico')
        
        # Clean up extra files
        for _, filename in extra_sizes:
            if os.path.exists(filename):
                os.remove(filename)
    
    # Create .icns file for macOS
    create_icns_file('128x128@2x.png', 'icon.icns')