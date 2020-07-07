use crate::backend::traits::{Backend, Emitable};
use crate::errors::{Error, Result};
use crate::frontend::{
    colors,
    colors::ColorHelper,
    pretty_print::{display, PrettyHelper},
};
use crate::lang::library::ast as lib;
use crate::lang::structure::Node;
use crate::lang::{
    ast,
    ast::{Atom, Cell, Control, GuardExpr, Port},
    component, context,
    structure::{DataDirection, NodeData},
};
use itertools::Itertools;
use lib::Implementation;
use petgraph::graph::NodeIndex;
use pretty::termcolor::ColorSpec;
use pretty::RcDoc;
use std::cmp::Ordering;
use std::io::Write;

type D<'a> = RcDoc<'a, ColorSpec>;

/// Implements a simple Verilog backend. The backend
/// only accepts Futil programs with no control and no groups.
pub struct VerilogBackend {}

/// Checks to make sure that there are no holes being
/// used in a guard.
fn validate_guard(guard: &GuardExpr) -> bool {
    match guard {
        GuardExpr::And(left, right) => {
            validate_guard(left) && validate_guard(right)
        }
        GuardExpr::Or(left, right) => {
            validate_guard(left) && validate_guard(right)
        }
        GuardExpr::Eq(left, right) => {
            validate_guard(left) && validate_guard(right)
        }
        GuardExpr::Neq(left, right) => {
            validate_guard(left) && validate_guard(right)
        }
        GuardExpr::Gt(left, right) => {
            validate_guard(left) && validate_guard(right)
        }
        GuardExpr::Lt(left, right) => {
            validate_guard(left) && validate_guard(right)
        }
        GuardExpr::Geq(left, right) => {
            validate_guard(left) && validate_guard(right)
        }
        GuardExpr::Leq(left, right) => {
            validate_guard(left) && validate_guard(right)
        }
        GuardExpr::Not(inner) => validate_guard(inner),
        GuardExpr::Atom(Atom::Port(p)) => {
            matches!(p, Port::Comp { .. } | Port::This { .. })
        }
        GuardExpr::Atom(Atom::Num(_)) => true,
    }
}

/// Returns `Ok` if there are no groups defined.
fn validate_structure(comp: &component::Component) -> Result<()> {
    let valid = comp.structure.edge_idx().all(|idx| {
        let edge = &comp.structure.graph[idx];
        edge.guard
            .as_ref()
            .map(|g| validate_guard(g))
            .unwrap_or(true)
            && edge.group.is_none()
    });
    if valid {
        Ok(())
    } else {
        Err(Error::MalformedStructure(
            "Groups / Holes can not be turned into Verilog".to_string(),
        ))
    }
}

/// Returns `Ok` if the control for `comp` is either a single `enable`
/// or `empty`.
fn validate_control(comp: &component::Component) -> Result<()> {
    match &comp.control {
        Control::Empty { .. } => Ok(()),
        _ => Err(Error::MalformedControl(
            "Must either be a single enable or an empty statement".to_string(),
        )),
    }
}

impl Backend for VerilogBackend {
    fn name() -> &'static str {
        "verilog"
    }

    fn validate(ctx: &context::Context) -> Result<()> {
        ctx.definitions_iter(|_, comp| {
            validate_structure(comp)?;
            validate_control(comp)
        })
    }

    fn emit<W: Write>(ctx: &context::Context, file: W) -> Result<()> {
        let prog: ast::NamespaceDef = ctx.clone().into();

        // build Vec of tuples first so that `comps` lifetime is longer than
        // `docs` lifetime
        let comps: Vec<(&ast::ComponentDef, component::Component)> = prog
            .components
            .iter()
            .map(|cd| (cd, ctx.get_component(&cd.name).unwrap()))
            .collect();

        let docs = comps
            .iter()
            .map(|(cd, comp)| cd.doc(&comp))
            .collect::<Result<Vec<_>>>()?;
        let prims = primitive_implemenations(&prog, ctx)?;
        display(
            colors::comment(D::text("/* verilator lint_off PINMISSING */"))
                .append(D::line())
                .append(prims)
                .append(D::line())
                .append(D::line())
                .append(D::intersperse(docs, D::line())),
            Some(file),
        );
        Ok(())
    }
}

/// Collects all of the Verilog implementations specified in the library
/// file.
fn primitive_implemenations<'a>(
    prog: &ast::NamespaceDef,
    context: &context::Context,
) -> Result<D<'a>> {
    let docs = prog
        .components
        .iter()
        .map(|c| c.cells.iter())
        .flatten()
        .filter_map(|s| match s {
            Cell::Prim { data } => Some(&data.instance.name),
            _ => None,
        })
        .unique()
        .map(|name| {
            context.library_context.definitions[&name]
                .implementation
                .iter()
                .find_map(|im| match im {
                    Implementation::Verilog { data } => {
                        Some(D::text(data.code.to_string()))
                    }
                })
                .ok_or_else(|| {
                    Error::MissingImplementation("Verilog", name.clone())
                })
        })
        .collect::<Result<Vec<_>>>()?;
    Ok(D::intersperse(docs, D::line().append(D::line())))
}

impl Emitable for ast::ComponentDef {
    fn doc<'a>(&self, comp: &component::Component) -> Result<D<'a>> {
        let structure = D::nil()
            .append(D::space())
            .append(self.name.to_string())
            .append(self.signature.doc(&comp)?)
            .append(";")
            .append(D::line())
            .append(D::line())
            .append(colors::comment(D::text("// Structure wire declarations")))
            .append(D::line())
            .append(wire_declarations(&comp)?)
            .append(D::line())
            .append(D::line())
            .append(colors::comment(D::text("// Subcomponent Instances")))
            .append(D::line())
            .append(subcomponent_instances(&comp))
            .append(D::line())
            .append(D::line())
            .append(colors::comment(D::text("// Input / output connections")))
            .append(D::line())
            .append(connections(&comp));
        let inner = structure;

        Ok(colors::comment(D::text("// Component Signature"))
            .append(D::line())
            .append(D::text("module").define_color())
            .append(inner.nest(2))
            .append(D::line())
            .append(D::text("endmodule").define_color())
            .append(D::space())
            .append(colors::comment(D::text(format!(
                "// end {}",
                self.name.to_string()
            )))))
    }
}

impl Emitable for ast::Signature {
    fn doc<'a>(&self, comp: &component::Component) -> Result<D<'a>> {
        let mut inputs = self
            .inputs
            .iter()
            .map(|pd| {
                Ok(D::text("input")
                    .keyword_color()
                    .append(D::space())
                    .append(pd.doc(&comp)?))
            })
            .collect::<Result<Vec<_>>>()?;
        let mut outputs = self
            .outputs
            .iter()
            .map(|pd| {
                Ok(D::text("output")
                    .keyword_color()
                    .append(D::space())
                    .append(pd.doc(&comp)?))
            })
            .collect::<Result<Vec<_>>>()?;
        let mut ports = vec![D::text("input")
            .keyword_color()
            .append(D::space())
            .append(D::text("wire").keyword_color())
            .append(D::space())
            .append("clk")];
        ports.append(&mut inputs);
        ports.append(&mut outputs);
        let doc =
            D::intersperse(ports.into_iter(), D::text(",").append(D::line()));
        Ok(D::space()
            .append(D::line().append(doc).nest(4).append(D::line()).parens()))
    }
}

impl Emitable for ast::Portdef {
    fn doc<'a>(&self, _ctx: &component::Component) -> Result<D<'a>> {
        Ok(D::text("wire")
            .keyword_color()
            .append(D::space())
            .append(bitwidth(self.width)?)
            .append(self.name.to_string()))
    }
}

//==========================================
//        Wire Declaration Functions
//==========================================
/// Generate all the wire declarations for `comp`
fn wire_declarations<'a>(comp: &component::Component) -> Result<D<'a>> {
    let wires = comp
        .structure
        .component_iterator()
        // filter for cells because we don't need to declare wires for ports
        .filter_map(|(_idx, node)| match &node.data {
            NodeData::Cell(_) => Some(node),
            _ => None,
        })
        // extract name, portdef from input / output of signature
        .map(|node| {
            node.signature
                .inputs
                .iter()
                .map(move |pd| (&node.name, pd))
                .chain(
                    node.signature
                        .outputs
                        .iter()
                        .map(move |pd| (&node.name, pd)),
                )
        })
        .flatten()
        // XXX(sam), definitely could use `test` here
        .map(|(name, portdef)| {
            Ok(D::text("wire")
                .keyword_color()
                .append(D::space())
                .append(bitwidth(portdef.width)?)
                .append(format!(
                    "{}_{}",
                    name.to_string(),
                    portdef.name.to_string()
                ))
                .append(";"))
        })
        .collect::<Result<Vec<_>>>()?;
    Ok(D::intersperse(wires, D::line()))
}

/// Generates a Verilog identifier for a Port.
///   * `Port::This(port)` => port
///   * `Port::Comp(comp, port)` => comp_port
///   * `Port::Hole` => unreachable!
fn wire_id_from_port<'a>(port: &Port) -> D<'a> {
    match port {
        Port::This { port } => D::text(port.to_string()),
        Port::Comp { component, port } => {
            D::text(format!("{}_{}", component.to_string(), port.to_string()))
        }
        Port::Hole { .. } => unreachable!(
            "This should have been caught in the validation checking"
        ),
    }
}

/// Generates a Verilog identifier for a (Node, String).
///  * NodeData::Cell(..) => name_port
///  * NodeData::Port => port
///  * NodeData::Hole => impossible!
///  * NodeData::Constant({width: w, value: v}) => w'dv
fn wire_id_from_node<'a>(node: &Node, port: String) -> D<'a> {
    match &node.data {
        NodeData::Cell(..) => {
            D::text(format!("{}_{}", node.name.to_string(), port))
        }
        NodeData::Port => D::text(port),
        NodeData::Hole(..) => unreachable!(
            "This should have been caught in the validation checking"
        ),
        NodeData::Constant(n) => D::text(format!("{}'d{}", n.width, n.val)),
    }
}

/// Converts a guarded edge into a Verilog string
fn guard<'a>(expr: &GuardExpr) -> D<'a> {
    match expr {
        GuardExpr::And(a, b) => D::nil()
            .append(guard(a).append(" & ").append(guard(b)))
            .parens(),
        GuardExpr::Or(a, b) => D::nil()
            .append(guard(a).append(" | ").append(guard(b)))
            .parens(),
        GuardExpr::Eq(a, b) => D::nil()
            .append(guard(a).append(" == ").append(guard(b)))
            .parens(),
        GuardExpr::Neq(a, b) => D::nil()
            .append(guard(a).append(" != ").append(guard(b)))
            .parens(),
        GuardExpr::Gt(a, b) => D::nil()
            .append(guard(a).append(" > ").append(guard(b)))
            .parens(),
        GuardExpr::Lt(a, b) => D::nil()
            .append(guard(a).append(" < ").append(guard(b)))
            .parens(),
        GuardExpr::Geq(a, b) => D::nil()
            .append(guard(a).append(" >= ").append(guard(b)))
            .parens(),
        GuardExpr::Leq(a, b) => D::nil()
            .append(guard(a).append(" <= ").append(guard(b)))
            .parens(),
        GuardExpr::Not(a) => D::text("!").append(guard(a)),
        GuardExpr::Atom(a) => atom(a),
    }
}

/// Converts ast::Atom to a verilog string
fn atom<'a>(atom: &Atom) -> D<'a> {
    match atom {
        Atom::Port(p) => match p {
            Port::Comp { component, port } => D::text(format!(
                "{}_{}",
                component.to_string(),
                port.to_string()
            )),
            Port::This { port } => D::text(port.to_string()),
            Port::Hole { .. } => unreachable!(
                "Holes should be caught in the backend validation."
            ),
        },
        Atom::Num(n) => D::text(format!("{}'d{}", n.width, n.val)),
    }
}

/// Turn u64 into a formatted Verilog bitwidth specifier.
pub fn bitwidth<'a>(width: u64) -> Result<D<'a>> {
    match width.cmp(&1) {
        Ordering::Less => unreachable!(),
        Ordering::Equal => Ok(D::nil()),
        Ordering::Greater => {
            Ok(D::text(format!("[{}:0]", width - 1)).append(D::space()))
        }
    }
}

//==========================================
//        Connection Functions
//==========================================
/// Generate wire connections
fn connections<'a>(comp: &component::Component) -> D<'a> {
    let doc = comp
        .structure
        .component_iterator()
        // for every component
        .map(|(idx, node)| {
            node.signature
                .inputs
                .iter()
                // get all the edges writing into a port
                .map(move |portdef| {
                    (
                        portdef.name.to_string(),
                        // collect all edges writing into this node and port
                        comp.structure
                            .edge_idx()
                            .with_direction(DataDirection::Write)
                            .with_node(idx)
                            .with_port(portdef.name.to_string())
                            .map(|idx| {
                                (
                                    comp.structure.graph[idx].clone(),
                                    comp.structure.get_node(
                                        comp.structure.endpoints(idx).0,
                                    ),
                                )
                            })
                            .collect::<Vec<_>>(),
                    )
                })
                // remove empty edges because we don't need to assign them anything
                .filter(|(_, edges)| !edges.is_empty())
                // fold all the edges into a single assign statement
                // with nested ternary statements to implement muxing
                .map(|(name, edges)| {
                    D::text("assign")
                        .keyword_color()
                        .append(D::space())
                        .append(wire_id_from_node(&node, name))
                        .append(" = ")
                        .append(edges.iter().rev().fold(
                            D::text("'0"),
                            |acc, (el, node)| {
                                el.guard
                                    .as_ref()
                                    .map(|g| guard(&g))
                                    .unwrap_or_else(D::nil)
                                    .append(" ? ")
                                    .append(wire_id_from_node(
                                        &node,
                                        el.src.port_name().to_string(),
                                    ))
                                    .append(" : ")
                                    .append(acc)
                                    .parens()
                            },
                        ))
                        .append(";")
                })
                .collect::<Vec<_>>()
        })
        .flatten();

    D::intersperse(doc, D::line())
}

//==========================================
//        Subcomponent Instance Functions
//==========================================
/// Generate Verilog for each subcomponent instanstiation and
/// wire up all the ports.
fn subcomponent_instances<'a>(comp: &component::Component) -> D<'a> {
    let doc = comp
        .structure
        .component_iterator()
        .filter_map(|(idx, node)| {
            if let NodeData::Cell(cell) = &node.data {
                Some((node, idx, cell))
            } else {
                None
            }
        })
        .map(|(node, idx, cell)| {
            subcomponent_sig(&node.name, &cell)
                .append(D::space())
                .append(
                    D::line()
                        .append(signature_connections(
                            &node.signature,
                            &comp,
                            idx,
                        ))
                        .nest(4)
                        .append(D::line())
                        .parens(),
                )
                .append(";")
        });
    D::intersperse(doc, D::line().append(D::line()))
}

/// Generates just the Verilog instanstiation code, but none
/// of the connections.
fn subcomponent_sig<'a>(id: &ast::Id, structure: &ast::Cell) -> D<'a> {
    let (name, params): (&ast::Id, &[u64]) = match structure {
        Cell::Decl { data } => (&data.component, &[]),
        Cell::Prim { data } => (&data.instance.name, &data.instance.params),
    };

    D::text(name.to_string())
        .append(D::line())
        .append("#")
        .append(
            D::intersperse(
                params.iter().map(|param| D::text(param.to_string())),
                D::text(",").append(D::line()),
            )
            .group()
            .parens(),
        )
        .append(D::line())
        .append(id.to_string())
        .group()
}

/// Generates Verilog for connection ports to wires.
fn signature_connections<'a>(
    sig: &ast::Signature,
    comp: &component::Component,
    idx: NodeIndex,
) -> D<'a> {
    // wire up all the incoming edges
    let incoming = sig
        .inputs
        .iter()
        .map(|portdef| {
            // if portdef is named `clk`, wire up `clk`
            if &portdef.name == "clk" {
                vec![D::text(".").append("clk").append(D::text("clk").parens())]
            } else {
                comp.structure
                    .edge_idx()
                    .with_direction(DataDirection::Write)
                    .with_node(idx)
                    .with_port(portdef.name.to_string())
                    .detach()
                    .map(|edge_idx| &comp.structure.graph[edge_idx])
                    // we only want one connection per dest
                    .unique_by(|edge| &edge.dest)
                    .map(move |edge| {
                        D::text(".")
                            .append(D::text(portdef.name.to_string()))
                            .append(wire_id_from_port(&edge.dest).parens())
                    })
                    .collect::<Vec<_>>()
            }
        })
        .flatten();

    // wire up outgoing edges
    let outgoing = sig.outputs.iter().map(|portdef| {
        D::text(".")
            .append(D::text(portdef.name.to_string()))
            .append(
                D::text(format!(
                    "{}_{}",
                    comp.structure.get_node(idx).name.to_string(),
                    portdef.name.to_string()
                ))
                .parens(),
            )
    });

    D::intersperse(incoming.chain(outgoing), D::text(",").append(D::line()))
}
