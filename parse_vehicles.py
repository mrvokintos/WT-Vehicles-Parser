#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""
War Thunder Vehicle Parser
Extracts vehicle information from War Thunder wiki HTML files and generates JSON databases

Usage:
    python parse_vehicles.py

The script will:
1. Parse wiki/ground.htm for ground vehicles
2. Parse wiki/aviation.htm for aircraft
3. Parse wiki/helicopters.htm for helicopters
4. Generate vehicles.json (combined database)
5. Generate wiki/ground_vehicles.json (ground only)
6. Generate wiki/aviation_vehicles.json (aviation only)
7. Generate wiki/helicopters_vehicles.json (helicopters only)

This script handles UTF-8 encoding issues commonly found in downloaded wiki files
and fixes mojibake sequences in vehicle names (decorative markers like ◄, ▄, etc.)
"""

import json
import re
import os
from pathlib import Path
from collections import defaultdict

try:
    from html.parser import HTMLParser
except:
    from HTMLParser import HTMLParser

# Mapping of nations to their directory prefixes and display names
NATION_MAP = {
    'usa': 'USA',
    'ussr': 'USSR',
    'germany': 'Germany',
    'great_britain': 'Great Britain',
    'britain': 'Great Britain',
    'japan': 'Japan',
    'china': 'China',
    'italy': 'Italy',
    'france': 'France',
    'sweden': 'Sweden',
    'israel': 'Israel',
}

# Mapping of vehicle types
TYPE_MAP = {
    # Ground vehicles
    'tank': 'tank',
    'medium_tank': 'tank',
    'heavy_tank': 'tank',
    'light_tank': 'tank',
    'tank_destroyer': 'tank',
    'spaa': 'zsu',
    'self_propelled_gun': 'tank',
    'anti_aircraft_gun': 'zsu',
    'medium_assault_tank': 'tank',
    'infantry_fighting_vehicle': 'tank',
    'armoured_car': 'tank',
    # Aviation
    'fighter': 'plane',
    'fighter_bomber': 'plane',
    'bomber': 'plane',
    'dive_bomber': 'plane',
    'jet_fighter': 'plane',
    'strike_aircraft': 'plane',
    'attack_aircraft': 'plane',
    'attackplane': 'plane',
    'assault_plane': 'plane',
    'reconnaissance': 'plane',
    'transport': 'plane',
    'jet_bomber': 'plane',
    'hydroplane': 'plane',
    'seaplane': 'plane',
    'interceptor': 'plane',
    'attacker': 'plane',
    'light_bomber': 'plane',
    'ground_attack': 'plane',
    'assault': 'plane',  # Ударный самолёт or helicopter (context dependent)
    # Helicopters
    'helicopter': 'heli',
    'attack_helicopter': 'heli',
    'utility_helicopter': 'heli',
    'transport_helicopter': 'heli',
}

def extract_vehicle_info(html_content, file_category='ground'):
    """Extract vehicle information from HTML content
    
    Args:
        html_content: HTML content to parse
        file_category: 'ground', 'aviation', or 'helicopters' for context-dependent type mapping
    """
    vehicles = defaultdict(lambda: defaultdict(list))
    
    # More robust regex for finding complete rows
    # Split by <tr class="wt-ulist_unit
    parts = re.split(r'<tr class="wt-ulist_unit', html_content)
    
    for part_idx in range(1, len(parts)):  # Skip first empty part
        part = '<tr class="wt-ulist_unit' + parts[part_idx]
        
        # Find the closing </tr>
        end_idx = part.find('</tr>')
        if end_idx == -1:
            continue
        row_html = part[:end_idx + 5]
        
        # Extract vehicle name from span inside wt-ulist_unit-name
        name_match = re.search(r'<td[^>]*class="wt-ulist_unit-name"[^>]*>.*?<span[^>]*>([^<]+)</span>', row_html, re.DOTALL)
        if not name_match:
            continue
        
        vehicle_name = name_match.group(1).strip()
        # Fix encoding issues first - these are very common UTF-8 mojibake sequences
        # Map of broken sequences to correct symbols
        encoding_fixes = {
            'в—Ќ': '◍',   # WHITE CIRCLE
            'в–„': '▄',   # LOWER HALF BLOCK
            'в—„': '◄',   # LEFT-POINTING TRIANGLE
            'вђ—': '␗',   # SYMBOL FOR DELETE
            'в—Љ': '◊',   # LOZENGE
            'в—"': '▄',   # LOWER HALF BLOCK (variant)
            'в–Ђ': '▀',   # UPPER HALF BLOCK
            'в—': '◄',    # LEFT-POINTING TRIANGLE (shorter sequence)
            'в–': '▄',    # LOWER HALF BLOCK (shorter sequence)
            'в„': '▅',    # LOWER THREE QUARTERS BLOCK
            'в„ќ': '─',   # BOX DRAWINGS LIGHT HORIZONTAL
            'в„љ': '│',   # BOX DRAWINGS LIGHT VERTICAL
        }
        
        # Apply fixes in order of longest sequences first to avoid partial replacements
        for broken, correct in sorted(encoding_fixes.items(), key=lambda x: len(x[0]), reverse=True):
            vehicle_name = vehicle_name.replace(broken, correct)
        
        # Only remove HTML entities
        vehicle_name = re.sub(r'&nbsp;', ' ', vehicle_name).strip()
        vehicle_name = re.sub(r'&[a-z]+;', '', vehicle_name).strip()
        vehicle_name = re.sub(r'\s+', ' ', vehicle_name).strip()
        
        if not vehicle_name or len(vehicle_name) < 2:
            continue
        
        # Extract nation
        nation_match = re.search(r'class="wt-ulist_unit-country"[^>]*data-value="([^"]+)"', row_html)
        if not nation_match:
            continue
        
        nation_key = nation_match.group(1)
        nation = NATION_MAP.get(nation_key, nation_key)
        
        # Extract vehicle type - use both absolute and relative paths
        type_match = re.search(r'class_icon/([^"\.#]+)(?:\.svg|\.svg#)', row_html)
        if not type_match:
            continue
        
        vehicle_type_key = type_match.group(1)
        
        # Context-dependent mapping for ambiguous types
        if file_category == 'helicopters':
            # For helicopters, map everything to 'heli'
            if vehicle_type_key in ['assault', 'bomber']:
                vehicle_type = 'heli'
            else:
                vehicle_type = TYPE_MAP.get(vehicle_type_key, 'heli')
        elif file_category == 'aviation':
            # For aviation, map to 'plane'
            if vehicle_type_key in ['assault', 'bomber', 'fighter']:
                vehicle_type = 'plane'
            else:
                vehicle_type = TYPE_MAP.get(vehicle_type_key, 'plane')
        else:
            # For ground vehicles, use default mapping
            vehicle_type = TYPE_MAP.get(vehicle_type_key, 'tank')
        
        # Store vehicle
        vehicles[nation][vehicle_type].append(vehicle_name)
    
    return vehicles

def parse_wiki_file(file_path, file_category='ground'):
    """Parse a single wiki HTML file and extract vehicle information
    
    Args:
        file_path: Path to the HTML file
        file_category: 'ground', 'aviation', or 'helicopters' for context-dependent type mapping
    """
    print(f"Reading {file_path.name}...")
    
    content = None
    # Try reading with UTF-8 first (correct encoding)
    try:
        with open(file_path, 'r', encoding='utf-8') as f:
            content = f.read()
    except Exception as e:
        print(f"  Error reading {file_path} as UTF-8: {e}")
        # Try binary read and decode
        try:
            with open(file_path, 'rb') as f:
                raw_bytes = f.read()
                # Try UTF-8 decode
                content = raw_bytes.decode('utf-8')
        except:
            try:
                # Fallback to latin-1
                with open(file_path, 'r', encoding='latin-1') as f:
                    content = f.read()
            except Exception as e:
                print(f"  Failed to read {file_path}")
                return None
    
    if content is None:
        print(f"  Error reading {file_path}: could not determine encoding")
        return None
    
    print(f"Parsing {file_path.name}...")
    vehicles = extract_vehicle_info(content, file_category)
    
    return vehicles

def build_json_structure(all_vehicles):
    """Build the final JSON structure"""
    result = {}
    
    for nation in sorted(all_vehicles.keys()):
        types = all_vehicles[nation]
        
        for vtype in ['tank', 'plane', 'heli', 'zsu']:
            if vtype in types:
                vehicles_of_type = types[vtype]
                # Remove duplicates while preserving order
                seen = set()
                unique_vehicles = []
                for v in vehicles_of_type:
                    if v not in seen:
                        seen.add(v)
                        unique_vehicles.append(v)
                
                for vehicle_name in sorted(unique_vehicles):
                    result[vehicle_name] = {
                        "nation": nation,
                        "type": vtype
                    }
    
    return result

def save_json_file(json_data, output_file, description):
    """Save JSON data to file with proper encoding"""
    with open(output_file, 'w', encoding='utf-8-sig') as f:
        json.dump(json_data, f, ensure_ascii=False, indent=2)
    print(f"  {description}: {len(json_data)} vehicles -> {output_file}")

def print_summary(json_data, title):
    """Print summary statistics for vehicle data"""
    summary = defaultdict(lambda: defaultdict(int))
    for name, info in json_data.items():
        summary[info['nation']][info['type']] += 1
    
    print(f"\n{title}:")
    for nation in sorted(summary.keys()):
        types_str = ', '.join([f"{vtype}: {summary[nation][vtype]}" 
                              for vtype in ['tank', 'plane', 'heli', 'zsu'] 
                              if vtype in summary[nation]])
        if types_str:
            print(f"  {nation}: {types_str}")

def main():
    # Get script directory and set paths relative to it
    script_dir = Path(__file__).parent.resolve()
    wiki_dir = script_dir / 'wiki'
    output_dir = script_dir
    
    print("=== War Thunder Vehicle Parser ===\n")
    print(f"Working directory: {script_dir}")
    
    # Create wiki directory if it doesn't exist
    if not wiki_dir.exists():
        print(f"\n⚠ Creating wiki directory: {wiki_dir}")
        wiki_dir.mkdir(parents=True, exist_ok=True)
    
    # Define files to process
    files_config = {
        'ground': {
            'file': wiki_dir / 'ground.htm',
            'output': wiki_dir / 'ground_vehicles.json',
            'description': 'Ground vehicles'
        },
        'aviation': {
            'file': wiki_dir / 'aviation.htm',
            'output': wiki_dir / 'aviation_vehicles.json',
            'description': 'Aviation'
        },
        'helicopters': {
            'file': wiki_dir / 'helicopters.htm',
            'output': wiki_dir / 'helicopters_vehicles.json',
            'description': 'Helicopters'
        }
    }
    
    # Check if any HTML files exist
    existing_files = [config['file'] for config in files_config.values() if config['file'].exists()]
    
    if not existing_files:
        print(f"\n❌ ERROR: No HTML files found in {wiki_dir}")
        print("\nPlease download the following HTML files from War Thunder Wiki:")
        print("  1. Ground vehicles → save as 'wiki/ground.htm'")
        print("  2. Aviation → save as 'wiki/aviation.htm'")
        print("  3. Helicopters → save as 'wiki/helicopters.htm'")
        print("\nWiki URL: https://wiki.warthunder.com/")
        return
    
    # Parse all files
    all_vehicles_by_category = {}
    combined_vehicles = defaultdict(lambda: defaultdict(list))
    
    for category, config in files_config.items():
        if not config['file'].exists():
            print(f"⚠ Warning: {config['file'].name} not found, skipping...")
            continue
        
        print(f"\n--- Processing {config['description']} ---")
        vehicles = parse_wiki_file(config['file'], category)
        
        if vehicles:
            all_vehicles_by_category[category] = vehicles
            
            # Build JSON for this category
            category_json = build_json_structure(vehicles)
            
            # Save category-specific JSON
            save_json_file(category_json, config['output'], config['description'])
            
            # Print summary for this category
            print_summary(category_json, f"{config['description']} summary")
            
            # Merge into combined vehicles
            for nation, types in vehicles.items():
                for vtype, names in types.items():
                    combined_vehicles[nation][vtype].extend(names)
    
    # Build and save combined JSON
    if combined_vehicles:
        print(f"\n--- Building combined database ---")
        combined_json = build_json_structure(combined_vehicles)
        combined_output = output_dir / 'vehicles.json'
        save_json_file(combined_json, combined_output, "Combined vehicles")
        
        # Print combined summary
        print_summary(combined_json, "Combined database summary")
    else:
        print("\n⚠ No vehicles found in any file!")
    
    print("\n=== Done! ===")

if __name__ == '__main__':
    main()
